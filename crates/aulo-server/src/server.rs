//! The server builder: mounts caller services next to health and reflection,
//! serves every bound endpoint with the same routes and limits, and shuts all
//! of them down together.

use std::convert::Infallible;
use std::fmt;

use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;
use tonic::body::Body;
use tonic::codegen::{Service, http};
use tonic::server::NamedService;
use tonic::service::{Routes, RoutesBuilder};
use tonic::transport::Server;
use tonic_health::ServingStatus;
use tonic_health::server::HealthReporter;

use crate::error::ServerError;
use crate::limits::{Limits, MessageLimits};
use crate::listener::{Bound, BoundKind};

/// The `aulod` API server. Holds no business logic: callers add the
/// `aulo.v1` services they implement.
pub struct ApiServer {
    limits: Limits,
    routes: RoutesBuilder,
    health: HealthReporter,
    services: Vec<&'static str>,
}

impl fmt::Debug for ApiServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ApiServer")
            .field("limits", &self.limits)
            .field("services", &self.services)
            .finish_non_exhaustive()
    }
}

impl ApiServer {
    /// A server with `grpc.health.v1` and reflection (v1 and v1alpha, since
    /// tools still differ in which one they ask for) already mounted.
    pub fn new(limits: Limits) -> Result<Self, ServerError> {
        let (health, health_service) = tonic_health::server::health_reporter();
        let reflection = || {
            tonic_reflection::server::Builder::configure()
                .register_encoded_file_descriptor_set(aulo_proto::FILE_DESCRIPTOR_SET)
                .register_encoded_file_descriptor_set(tonic_health::pb::FILE_DESCRIPTOR_SET)
        };
        let mut routes = Routes::builder();
        routes
            .add_service(health_service.with_message_limits(&limits))
            .add_service(reflection().build_v1()?.with_message_limits(&limits))
            .add_service(reflection().build_v1alpha()?.with_message_limits(&limits));
        Ok(Self {
            limits,
            routes,
            health,
            services: Vec::new(),
        })
    }

    /// Mounts a generated service with this server's message limits applied.
    /// Its health status reads `SERVING` while the server runs.
    #[must_use]
    pub fn add_service<S>(mut self, service: S) -> Self
    where
        S: MessageLimits
            + Service<http::Request<Body>, Response = http::Response<Body>, Error = Infallible>
            + NamedService
            + Clone
            + Send
            + Sync
            + 'static,
        S::Future: Send + 'static,
    {
        self.routes
            .add_service(service.with_message_limits(&self.limits));
        self.services.push(S::NAME);
        self
    }

    /// The health reporter, for services that want to flip their own status.
    pub fn health(&self) -> HealthReporter {
        self.health.clone()
    }

    /// Serves every endpoint until `shutdown` is cancelled or one endpoint
    /// fails, which stops the others too.
    ///
    /// On shutdown health turns `NOT_SERVING`, new connections stop, and
    /// open streams get `Limits::shutdown_grace` to finish. Streams still
    /// open after that are abandoned: their tasks end with the runtime when
    /// `aulod` exits, so a stuck client cannot hold the daemon up.
    pub async fn serve(
        self,
        listeners: Vec<Bound>,
        shutdown: CancellationToken,
    ) -> Result<(), ServerError> {
        for name in std::iter::once("").chain(self.services.iter().copied()) {
            self.health
                .set_service_status(name, ServingStatus::Serving)
                .await;
        }
        let stop = shutdown.child_token();
        let routes = self.routes.routes();
        let mut tasks = JoinSet::new();
        for listener in listeners {
            tasks.spawn(serve_one(
                self.limits,
                routes.clone(),
                listener,
                stop.clone(),
            ));
        }

        let health = self.health.clone();
        let services = self.services.clone();
        let grace = self.limits.shutdown_grace;
        let deadline = async {
            stop.cancelled().await;
            for name in std::iter::once("").chain(services) {
                health
                    .set_service_status(name, ServingStatus::NotServing)
                    .await;
            }
            tokio::time::sleep(grace).await;
        };
        tokio::pin!(deadline);

        let mut result = Ok(());
        loop {
            tokio::select! {
                joined = tasks.join_next() => {
                    let failure = match joined {
                        None => break,
                        Some(Ok(Ok(()))) => continue,
                        Some(Ok(Err(e))) => e,
                        Some(Err(e)) => ServerError::Task(e),
                    };
                    tracing::error!(error = %failure, "an API listener failed; stopping the server");
                    if result.is_ok() {
                        result = Err(failure);
                    }
                    stop.cancel();
                }
                () = &mut deadline => {
                    tracing::warn!(?grace, "streams still open after the shutdown grace; abandoning them");
                    tasks.abort_all();
                    break;
                }
            }
        }
        result
    }
}

fn builder(limits: Limits) -> Server {
    Server::builder()
        .concurrency_limit_per_connection(limits.concurrency_limit_per_connection)
        .max_concurrent_streams(limits.max_concurrent_streams)
        .timeout(limits.request_timeout)
}

// T3.4 adds authentication here: the local socket trusts its owner, the TCP
// branch gets the token interceptor (and T3.8 the TLS config).
async fn serve_one(
    limits: Limits,
    routes: Routes,
    listener: Bound,
    stop: CancellationToken,
) -> Result<(), ServerError> {
    let signal = stop.clone().cancelled_owned();
    let router = builder(limits).add_routes(routes);
    match listener.0 {
        #[cfg(unix)]
        BoundKind::Unix(listener, _file) => {
            let incoming = tokio_stream::wrappers::UnixListenerStream::new(listener);
            router
                .serve_with_incoming_shutdown(incoming, signal)
                .await?;
        }
        #[cfg(windows)]
        BoundKind::NamedPipe(first, name) => {
            let incoming = crate::pipe::incoming(first, name, stop);
            router
                .serve_with_incoming_shutdown(incoming, signal)
                .await?;
        }
        BoundKind::Tcp(incoming, _) => {
            router
                .serve_with_incoming_shutdown(incoming, signal)
                .await?;
        }
    }
    Ok(())
}
