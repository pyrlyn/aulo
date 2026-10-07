//! Transport limits and the trait that applies the message-size caps to every
//! service the server mounts.
//!
//! tonic sets message sizes per generated service, not on the server, so a
//! service added without them would accept tonic's defaults (an unbounded
//! response size). [`MessageLimits`] is required by `ApiServer::add_service`,
//! which makes forgetting the caps a compile error.

use std::time::Duration;

use aulo_proto::aulo::v1;

/// Caps on what one client can make the server hold or do.
///
/// The defaults suit a single-user local daemon; remote mode will want its
/// own values once rate limits exist (spec §13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Largest message a client may send. Requests above it fail with
    /// `OUT_OF_RANGE` before a handler sees them.
    pub max_decoding_message_size: usize,
    /// Largest message the server will send.
    pub max_encoding_message_size: usize,
    /// In-flight request handlers per connection.
    pub concurrency_limit_per_connection: usize,
    /// Open HTTP/2 streams per connection, which bounds long-lived streams
    /// that the concurrency limit no longer counts once they start.
    pub max_concurrent_streams: u32,
    /// Deadline for a handler to produce its response. A shorter
    /// `grpc-timeout` from the client wins.
    pub request_timeout: Duration,
    /// How long shutdown waits for open streams before abandoning them.
    pub shutdown_grace: Duration,
    /// How long a TCP client may take to finish the TLS handshake. tonic runs
    /// each handshake in its own task with no deadline, so without this a
    /// client that connects and goes silent holds that task forever.
    pub tls_handshake_timeout: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_decoding_message_size: 4 * 1024 * 1024,
            max_encoding_message_size: 4 * 1024 * 1024,
            concurrency_limit_per_connection: 32,
            max_concurrent_streams: 64,
            request_timeout: Duration::from_secs(30),
            shutdown_grace: Duration::from_secs(10),
            tls_handshake_timeout: Duration::from_secs(10),
        }
    }
}

/// A generated tonic service whose message sizes can be capped.
pub trait MessageLimits {
    #[must_use]
    fn with_message_limits(self, limits: &Limits) -> Self;
}

// The generated servers only share these methods by name, not by a trait.
macro_rules! message_limits {
    ($($server:ty),* $(,)?) => {$(
        impl<T> MessageLimits for $server {
            fn with_message_limits(self, limits: &Limits) -> Self {
                self.max_decoding_message_size(limits.max_decoding_message_size)
                    .max_encoding_message_size(limits.max_encoding_message_size)
            }
        }
    )*};
}

message_limits!(
    v1::approval_service_server::ApprovalServiceServer<T>,
    v1::audit_service_server::AuditServiceServer<T>,
    v1::chat_service_server::ChatServiceServer<T>,
    v1::config_service_server::ConfigServiceServer<T>,
    v1::mcp_service_server::McpServiceServer<T>,
    v1::plugin_service_server::PluginServiceServer<T>,
    v1::session_service_server::SessionServiceServer<T>,
    v1::voice_service_server::VoiceServiceServer<T>,
    tonic_health::pb::health_server::HealthServer<T>,
    tonic_reflection::pb::v1::server_reflection_server::ServerReflectionServer<T>,
    tonic_reflection::pb::v1alpha::server_reflection_server::ServerReflectionServer<T>,
);
