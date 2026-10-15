//! `aulo.v1.ConfigService` for provider keys and the effective configuration.
//!
//! `SetProviderKey` writes the OS keychain (or the injected store) and never
//! returns the key. `ListProviders` reports only whether a key is available.
//! `GetConfig` is the redacted configuration JSON, which has no key field.

use std::sync::Arc;

use aulo_config::{Config, CredentialsError, SecretStore, configured_keys, mask, store_key};
use aulo_proto::aulo::v1::config_service_server::{ConfigService, ConfigServiceServer};
use aulo_proto::aulo::v1::{
    GetConfigRequest, GetConfigResponse, ListAudioDevicesRequest, ListAudioDevicesResponse,
    ListModelsRequest, ListModelsResponse, ListProvidersRequest, ListProvidersResponse, Provider,
    SetConfigRequest, SetConfigResponse, SetDeviceRequest, SetDeviceResponse,
    SetProviderKeyRequest, SetProviderKeyResponse,
};
use tonic::{Request, Response, Status};

/// Serves configuration reads and write-only provider keys.
///
/// Clone-cheap: clones share the config snapshot and the secret store.
#[derive(Clone)]
pub struct ConfigApi {
    config: Arc<Config>,
    secrets: Arc<dyn SecretStore>,
}

impl std::fmt::Debug for ConfigApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigApi").finish_non_exhaustive()
    }
}

impl ConfigApi {
    #[must_use]
    pub fn new(config: Config, secrets: Arc<dyn SecretStore>) -> Self {
        Self {
            config: Arc::new(config),
            secrets,
        }
    }

    /// The tonic service `ApiServer::add_service` mounts.
    #[must_use]
    pub fn service(self) -> ConfigServiceServer<Self> {
        ConfigServiceServer::new(self)
    }
}

fn unavailable(rpc: &str) -> Status {
    Status::unimplemented(format!("{rpc} is not available yet"))
}

fn status_of(error: CredentialsError) -> Status {
    if error.is_keychain_disabled() {
        return Status::failed_precondition(error.to_string());
    }
    match error {
        CredentialsError::BadId | CredentialsError::Empty | CredentialsError::TooLong => {
            Status::invalid_argument(error.to_string())
        }
        other => {
            tracing::error!(error = %other, "provider key store failed");
            Status::internal("cannot store the provider key")
        }
    }
}

#[tonic::async_trait]
impl ConfigService for ConfigApi {
    async fn list_providers(
        &self,
        _request: Request<ListProvidersRequest>,
    ) -> Result<Response<ListProvidersResponse>, Status> {
        let config = Arc::clone(&self.config);
        let secrets = Arc::clone(&self.secrets);
        let providers = tokio::task::spawn_blocking(move || {
            let listed = configured_keys(&config, secrets.as_ref())?;
            let providers = listed
                .into_iter()
                .map(|info| {
                    let base_url = config
                        .providers
                        .endpoints
                        .get(&info.id)
                        .and_then(|endpoint| endpoint.base_url.as_deref().map(mask));
                    Provider {
                        id: info.id,
                        kind: info.kind.as_str().to_owned(),
                        base_url,
                        key_set: info.state.is_set(),
                    }
                })
                .collect();
            Ok::<_, CredentialsError>(providers)
        })
        .await
        .map_err(|error| {
            tracing::error!(error = %error, "provider key lookup task failed");
            Status::internal("cannot list providers")
        })?
        .map_err(status_of)?;
        Ok(Response::new(ListProvidersResponse { providers }))
    }

    async fn set_provider_key(
        &self,
        request: Request<SetProviderKeyRequest>,
    ) -> Result<Response<SetProviderKeyResponse>, Status> {
        let req = request.into_inner();
        let provider_id = req.provider_id;
        let key = req.key;
        let logged_id = provider_id.clone();
        let secrets = Arc::clone(&self.secrets);
        tokio::task::spawn_blocking(move || store_key(&provider_id, &key, secrets.as_ref()))
            .await
            .map_err(|error| {
                tracing::error!(error = %error, "provider key store task failed");
                Status::internal("cannot store the provider key")
            })?
            .map_err(status_of)?;
        tracing::info!(provider_id = %logged_id, "stored a provider key");
        Ok(Response::new(SetProviderKeyResponse {}))
    }

    async fn list_models(
        &self,
        _request: Request<ListModelsRequest>,
    ) -> Result<Response<ListModelsResponse>, Status> {
        Err(unavailable("ListModels"))
    }

    async fn list_audio_devices(
        &self,
        _request: Request<ListAudioDevicesRequest>,
    ) -> Result<Response<ListAudioDevicesResponse>, Status> {
        Err(unavailable("ListAudioDevices"))
    }

    async fn set_device(
        &self,
        _request: Request<SetDeviceRequest>,
    ) -> Result<Response<SetDeviceResponse>, Status> {
        Err(unavailable("SetDevice"))
    }

    async fn get_config(
        &self,
        _request: Request<GetConfigRequest>,
    ) -> Result<Response<GetConfigResponse>, Status> {
        let config_json = self.config.to_json_redacted().map_err(|error| {
            tracing::error!(error = %error, "cannot render config");
            Status::internal("cannot render the configuration")
        })?;
        Ok(Response::new(GetConfigResponse { config_json }))
    }

    async fn set_config(
        &self,
        _request: Request<SetConfigRequest>,
    ) -> Result<Response<SetConfigResponse>, Status> {
        Err(unavailable("SetConfig"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aulo_config::{Config, MemoryStore, ProviderConfig, ProviderKind, SecretStore};
    use aulo_proto::aulo::v1::config_service_server::ConfigService;
    use tonic::{Code, Request};

    use super::ConfigApi;

    const SECRET: &str = "sk-live-t42-9f3c1a7e-NEVER-RETURN";

    fn config() -> Config {
        let mut config = Config::default();
        config.providers.endpoints.insert(
            "openai".into(),
            ProviderConfig {
                kind: ProviderKind::Openai,
                base_url: Some("https://user:s3cret@api.openai.com/v1".into()),
            },
        );
        config
    }

    fn api(store: MemoryStore) -> ConfigApi {
        ConfigApi::new(config(), Arc::new(store) as Arc<dyn SecretStore>)
    }

    #[tokio::test]
    async fn set_provider_key_never_comes_back() {
        let store = MemoryStore::default();
        let api = api(store);
        api.set_provider_key(Request::new(aulo_proto::aulo::v1::SetProviderKeyRequest {
            provider_id: "openai".into(),
            key: SECRET.into(),
        }))
        .await
        .unwrap();

        let listed = api
            .list_providers(Request::new(aulo_proto::aulo::v1::ListProvidersRequest {}))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(listed.providers.len(), 1);
        assert!(listed.providers[0].key_set);
        assert_eq!(listed.providers[0].kind, "openai");
        let rendered = format!("{listed:?}");
        assert!(!rendered.contains(SECRET), "{rendered}");
        assert!(!rendered.contains("s3cret"), "{rendered}");

        let body = api
            .get_config(Request::new(aulo_proto::aulo::v1::GetConfigRequest {}))
            .await
            .unwrap()
            .into_inner();
        assert!(!body.config_json.contains(SECRET), "{}", body.config_json);
        assert!(!body.config_json.contains("s3cret"), "{}", body.config_json);
        assert!(
            !body.config_json.contains("api_key"),
            "{}",
            body.config_json
        );
        assert!(
            body.config_json.contains("[REDACTED]"),
            "{}",
            body.config_json
        );
    }

    #[tokio::test]
    async fn a_rejected_key_is_not_echoed() {
        let api = api(MemoryStore::default());
        let empty = api
            .set_provider_key(Request::new(aulo_proto::aulo::v1::SetProviderKeyRequest {
                provider_id: "openai".into(),
                key: "  ".into(),
            }))
            .await
            .unwrap_err();
        assert_eq!(empty.code(), Code::InvalidArgument);
        assert!(!empty.to_string().contains(SECRET), "{empty}");

        let huge = api
            .set_provider_key(Request::new(aulo_proto::aulo::v1::SetProviderKeyRequest {
                provider_id: "bad id".into(),
                key: SECRET.into(),
            }))
            .await
            .unwrap_err();
        assert_eq!(huge.code(), Code::InvalidArgument);
        assert!(!huge.to_string().contains(SECRET), "{huge}");
    }

    #[tokio::test]
    async fn list_models_is_not_this_task() {
        let api = api(MemoryStore::default());
        let status = api
            .list_models(Request::new(
                aulo_proto::aulo::v1::ListModelsRequest::default(),
            ))
            .await
            .unwrap_err();
        assert_eq!(status.code(), Code::Unimplemented);
    }
}
