//! `aulo.v1.ConfigService` for provider keys, models and the effective configuration.
//!
//! `SetProviderKey` writes the OS keychain (or the injected store) and never
//! returns the key. `ListProviders` reports only whether a key is available.
//! `ListModels` is the built-in catalog for each configured endpoint, with
//! the served overlay from startup. `GetConfig` is the redacted configuration
//! JSON, which has no key field.

use std::sync::Arc;

use aulo_config::{
    Config, CredentialsError, SecretStore, configured_keys, mask, store_key, validate_provider_id,
};
use aulo_proto::aulo::v1::config_service_server::{ConfigService, ConfigServiceServer};
use aulo_proto::aulo::v1::{
    GetConfigRequest, GetConfigResponse, ListAudioDevicesRequest, ListAudioDevicesResponse,
    ListModelsRequest, ListModelsResponse, ListProvidersRequest, ListProvidersResponse, Model,
    ModelRef, Provider, SetConfigRequest, SetConfigResponse, SetDeviceRequest, SetDeviceResponse,
    SetProviderKeyRequest, SetProviderKeyResponse,
};
use aulo_providers::{ModelCatalog, ServedModels, probe_served};
use tonic::{Request, Response, Status};

/// Serves configuration reads and write-only provider keys.
///
/// Clone-cheap: clones share the config snapshot and the secret store.
#[derive(Clone)]
pub struct ConfigApi {
    config: Arc<Config>,
    secrets: Arc<dyn SecretStore>,
    catalog: Arc<ModelCatalog>,
    served: Arc<ServedModels>,
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
            catalog: Arc::new(ModelCatalog::load()),
            served: Arc::new(ServedModels::default()),
        }
    }

    /// Replaces the served overlay. Tests inject a snapshot; the daemon uses
    /// [`Self::probe_local_models`] instead.
    #[must_use]
    pub fn with_served(mut self, served: ServedModels) -> Self {
        self.served = Arc::new(served);
        self
    }

    /// Probes local servers once, at startup. A server that is down leaves
    /// that endpoint's overlay empty; the catalog rows are still listed.
    pub async fn probe_local_models(self) -> Self {
        let served = probe_served(&self.config.providers).await;
        self.with_served(served)
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

const DEFAULT_PAGE_SIZE: usize = 50;
const MAX_PAGE_SIZE: usize = 200;
const MAX_TOKEN_BYTES: usize = 256;

/// Page size 0 means the default; anything above the cap is clamped. Same
/// numbers as ChatService, so a client can use one page size for both.
fn model_page_size(requested: u32) -> usize {
    match usize::try_from(requested).unwrap_or(MAX_PAGE_SIZE) {
        0 => DEFAULT_PAGE_SIZE,
        n => n.min(MAX_PAGE_SIZE),
    }
}

/// Position after a model in `(provider_id, model_id)` order. The length
/// prefix keeps a model id that contains `.` unambiguous.
#[derive(Debug, PartialEq, Eq)]
struct ModelCursor {
    provider: String,
    model: String,
}

impl ModelCursor {
    fn encode(provider: &str, model: &str) -> String {
        format!("k1.{}.{provider}{model}", provider.len())
    }

    fn parse(token: &str) -> Result<Option<Self>, Status> {
        if token.is_empty() {
            return Ok(None);
        }
        if token.len() > MAX_TOKEN_BYTES {
            return Err(Status::invalid_argument("page_token is not valid"));
        }
        let Some(body) = token.strip_prefix("k1.") else {
            return Err(Status::invalid_argument("page_token is not valid"));
        };
        let Some((len, rest)) = body.split_once('.') else {
            return Err(Status::invalid_argument("page_token is not valid"));
        };
        let Ok(len) = len.parse::<usize>() else {
            return Err(Status::invalid_argument("page_token is not valid"));
        };
        if len == 0 || len > rest.len() || len > 64 {
            return Err(Status::invalid_argument("page_token is not valid"));
        }
        let (provider, model) = rest.split_at(len);
        if validate_provider_id(provider).is_err()
            || model.is_empty()
            || model.len() > 128
            || model.chars().any(char::is_control)
        {
            return Err(Status::invalid_argument("page_token is not valid"));
        }
        Ok(Some(Self {
            provider: provider.to_owned(),
            model: model.to_owned(),
        }))
    }
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
        request: Request<ListModelsRequest>,
    ) -> Result<Response<ListModelsResponse>, Status> {
        let req = request.into_inner();
        let filter = match req.provider_id.as_deref().filter(|id| !id.is_empty()) {
            Some(id) => Some(validate_provider_id(id).map_err(status_of)?.to_owned()),
            None => None,
        };
        let size = model_page_size(req.page_size);
        let after = ModelCursor::parse(&req.page_token)?;
        let mut rows = self.catalog.list(&self.config.providers, &self.served);
        if let Some(id) = filter.as_deref() {
            rows.retain(|row| row.provider_id == id);
        }
        let start = match &after {
            None => 0,
            Some(cursor) => rows
                .iter()
                .position(|row| {
                    (row.provider_id.as_str(), row.model_id.as_str())
                        > (cursor.provider.as_str(), cursor.model.as_str())
                })
                .unwrap_or(rows.len()),
        };
        let mut page: Vec<_> = rows
            .into_iter()
            .skip(start)
            .take(size.saturating_add(1))
            .collect();
        let next_page_token = if page.len() > size {
            page.truncate(size);
            page.last()
                .map(|row| ModelCursor::encode(&row.provider_id, &row.model_id))
                .unwrap_or_default()
        } else {
            String::new()
        };
        let models = page
            .into_iter()
            .map(|row| Model {
                r#ref: Some(ModelRef {
                    provider: row.provider_id,
                    model: row.model_id,
                }),
                display_name: row.display_name,
                served: row.served,
            })
            .collect();
        Ok(Response::new(ListModelsResponse {
            models,
            next_page_token,
        }))
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

    fn listed(config: Config, served: aulo_providers::ServedModels) -> ConfigApi {
        ConfigApi::new(
            config,
            Arc::new(MemoryStore::default()) as Arc<dyn SecretStore>,
        )
        .with_served(served)
    }

    #[tokio::test]
    async fn list_models_merges_the_catalog_with_the_served_overlay() {
        let mut config = config();
        config.providers.endpoints.insert(
            "anthropic".into(),
            aulo_config::ProviderConfig {
                kind: ProviderKind::Anthropic,
                base_url: None,
            },
        );
        config.providers.endpoints.insert(
            "ollama".into(),
            aulo_config::ProviderConfig {
                kind: ProviderKind::Ollama,
                base_url: None,
            },
        );
        config.providers.tiers.main = Some(aulo_config::TierConfig {
            provider: "ollama".into(),
            model: "llama3.2".into(),
        });
        let mut served = aulo_providers::ServedModels::default();
        served.insert("ollama", vec!["llama3.2".into(), "served-only".into()]);
        let api = listed(config, served);

        let page = api
            .list_models(Request::new(aulo_proto::aulo::v1::ListModelsRequest {
                provider_id: Some("anthropic".into()),
                page_size: 0,
                page_token: String::new(),
            }))
            .await
            .unwrap()
            .into_inner();
        let ids: Vec<_> = page
            .models
            .iter()
            .map(|model| {
                let model_ref = model.r#ref.as_ref().unwrap();
                assert_eq!(model_ref.provider, "anthropic");
                assert!(!model.served);
                model_ref.model.clone()
            })
            .collect();
        assert_eq!(
            ids,
            [
                "claude-fable-5-1",
                "claude-haiku-4-5",
                "claude-opus-5",
                "claude-sonnet-5",
            ]
        );
        let sonnet = page
            .models
            .iter()
            .find(|model| model.r#ref.as_ref().unwrap().model == "claude-sonnet-5")
            .unwrap();
        assert_eq!(sonnet.display_name, "Claude Sonnet 5");

        let local = api
            .list_models(Request::new(aulo_proto::aulo::v1::ListModelsRequest {
                provider_id: Some("ollama".into()),
                ..Default::default()
            }))
            .await
            .unwrap()
            .into_inner();
        let local_ids: Vec<_> = local
            .models
            .iter()
            .map(|model| {
                let model_ref = model.r#ref.as_ref().unwrap();
                (model_ref.model.as_str(), model.served)
            })
            .collect();
        assert_eq!(local_ids, [("llama3.2", true), ("served-only", true)]);

        let first = api
            .list_models(Request::new(aulo_proto::aulo::v1::ListModelsRequest {
                provider_id: Some("anthropic".into()),
                page_size: 1,
                page_token: String::new(),
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(first.models.len(), 1);
        assert!(!first.next_page_token.is_empty());
        let second = api
            .list_models(Request::new(aulo_proto::aulo::v1::ListModelsRequest {
                provider_id: Some("anthropic".into()),
                page_size: 1,
                page_token: first.next_page_token,
            }))
            .await
            .unwrap()
            .into_inner();
        assert_eq!(
            second.models[0].r#ref.as_ref().unwrap().model,
            "claude-haiku-4-5"
        );

        let bad = api
            .list_models(Request::new(aulo_proto::aulo::v1::ListModelsRequest {
                page_token: "nope".into(),
                ..Default::default()
            }))
            .await
            .unwrap_err();
        assert_eq!(bad.code(), Code::InvalidArgument);

        let unknown = api
            .list_models(Request::new(aulo_proto::aulo::v1::ListModelsRequest {
                provider_id: Some("missing".into()),
                ..Default::default()
            }))
            .await
            .unwrap()
            .into_inner();
        assert!(unknown.models.is_empty());

        let rejected = api
            .list_models(Request::new(aulo_proto::aulo::v1::ListModelsRequest {
                provider_id: Some("has space".into()),
                ..Default::default()
            }))
            .await
            .unwrap_err();
        assert_eq!(rejected.code(), Code::InvalidArgument);
        assert!(!rejected.to_string().contains("has space"));
    }
}
