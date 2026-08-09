use crate::MapKursalResult;
use crate::{
    KursalError,
    api::{CoreCommand, cmd_wrapper::StateWrapper},
    apiserver::iprecord::IpRecord,
    dto::{ContactResponse, MessageResponse, NearbyPeerResponse, OtpResponse},
    network::NetworkManager,
    storage::{Database, SharedDatabase},
};
use auth::auth_middleware;
use axum::{
    Router, middleware,
    routing::{any, delete, get, patch, post},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    net::{IpAddr, SocketAddr},
    sync::Arc,
};
use tokio::sync::{Mutex, MutexGuard, broadcast, mpsc, oneshot};
use tower_http::cors::CorsLayer;
use utoipa::{
    Modify, OpenApi,
    openapi::{
        ContentBuilder, Ref, RefOr, ResponseBuilder,
        security::{HttpAuthScheme, HttpBuilder, SecurityScheme},
    },
};
use utoipa_scalar::{Scalar, Servable};

pub mod auth;
mod handlers;
pub mod iprecord;
mod types;
mod ws;

use handlers::*;
use types::*;
use ws::*;

pub use types::APIError;

pub(crate) type Result<T> = std::result::Result<T, APIError>;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalApiConfig {
    pub enabled: bool,
    pub host_on_network: bool,
    pub port: u16,
}
impl LocalApiConfig {
    pub fn serialize(&self) -> crate::Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> crate::Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

impl Default for LocalApiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host_on_network: false,
            port: 4892,
        }
    }
}

#[derive(Clone)]
pub struct CoreEventEmitter {
    pub event: String,
    pub payload: serde_json::Value,
}

#[derive(Clone)]
pub struct APIAppState {
    auth_token: String,
    rate_map: Arc<Mutex<HashMap<IpAddr, IpRecord>>>,
    //
    db: SharedDatabase,
    core_cmd_tx: mpsc::Sender<CoreCommand>,
    network: Arc<Mutex<NetworkManager>>,
    pending_nearby: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    event_tx: broadcast::Sender<CoreEventEmitter>,
    app_data_dir: std::path::PathBuf,
}

impl StateWrapper for APIAppState {
    fn core_cmd_tx(&self) -> &mpsc::Sender<CoreCommand> {
        &self.core_cmd_tx
    }
    async fn network_lock(&self) -> MutexGuard<'_, NetworkManager> {
        self.network.lock().await
    }
    async fn pending_nearby_lock(&self) -> MutexGuard<'_, HashMap<String, oneshot::Sender<bool>>> {
        self.pending_nearby.lock().await
    }
    async fn db_lock(&self) -> MutexGuard<'_, Database> {
        self.db.0.lock().await
    }
}

struct BearerAuth;
impl Modify for BearerAuth {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearerAuth",
                SecurityScheme::Http(
                    HttpBuilder::new()
                        .scheme(HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build(),
                ),
            );
        }
    }
}

struct AuthResponses;
impl Modify for AuthResponses {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let response = |description: &str| {
            RefOr::T(
                ResponseBuilder::new()
                    .description(description)
                    .content(
                        "application/json",
                        ContentBuilder::new()
                            .schema(Some(Ref::from_schema_name("APIError")))
                            .build(),
                    )
                    .build(),
            )
        };

        for path in openapi.paths.paths.values_mut() {
            let operations = [
                path.get.as_mut(),
                path.put.as_mut(),
                path.post.as_mut(),
                path.delete.as_mut(),
                path.options.as_mut(),
                path.head.as_mut(),
                path.patch.as_mut(),
                path.trace.as_mut(),
            ];

            for operation in operations.into_iter().flatten() {
                operation
                    .responses
                    .responses
                    .insert("401".to_string(), response("Unauthorized"));
                operation
                    .responses
                    .responses
                    .insert("429".to_string(), response("Too many failed attempts"));
            }
        }
    }
}

#[derive(utoipa::OpenApi)]
#[openapi(
    info(
        title = "Kursal API",
        description = "An API for interacting with Kursal. You can enable the API Server in your app in \"Settings > Advanced > Local API Server > Enable.\""
    ),
    modifiers(&BearerAuth, &AuthResponses),
    security(("bearerAuth" = [])),
    tags(
        (name = "Self",       description = "Current user identity & profile"),
        (name = "OTP",        description = "One-time password operations"),
        (name = "LTC",        description = "Long-term credential import/export"),
        (name = "Nearby",     description = "Nearby peer discovery & connection"),
        (name = "Contacts",   description = "Contact management & blocking"),
        (name = "Messages",   description = "Manage messages"),
        (name = "Events",     description = "Real-time core event stream"),
    ),
    paths(
        ws_handler,
        api_self_get_user_id, api_self_get_profile, api_self_post_profile, api_self_get_peer_id, api_self_rotate_peer_id,
        api_otp_generate, api_otp_fetch,
        api_ltc_export, api_ltc_import,
        api_nearby_start, api_nearby_stop, api_nearby_get, api_nearby_connect, api_nearby_accept, api_nearby_decline,
        api_contacts, api_contact_security_code, api_contact_security_code_confirm, api_contact_share_profile, api_contact_get, api_contact_remove, api_contact_blocked_list, api_contact_block, api_contact_unblock,
        api_typing, api_messages_send, api_messages_get, api_message_delete_local, api_message_delete, api_message_pin, api_message_unpin, api_message_edit, api_message_reaction_add, api_message_reaction_remove, api_files_send, api_files_accept,
    ),
    components(schemas(
        APIError, APIEvent, APINearbyConnectMethod, APIMessageSend, APIMessageEdit, APISelfProfile, OtpResponse, ContactResponse, NearbyPeerResponse, MessageResponse, APIFile, APIFileDetails,
    )),
)]
pub struct ApiDoc;

#[allow(clippy::too_many_arguments)]
pub async fn run_server(
    auth_token: String,
    api_config: LocalApiConfig,
    core_cmd_tx: mpsc::Sender<CoreCommand>,
    db: SharedDatabase,
    network: Arc<Mutex<NetworkManager>>,
    pending_nearby: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
    event_tx: broadcast::Sender<CoreEventEmitter>,
    app_data_dir: std::path::PathBuf,
) -> crate::Result<()> {
    let host = if api_config.host_on_network {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };

    let mut openapi = ApiDoc::openapi();
    openapi.servers = Some(vec![
        utoipa::openapi::ServerBuilder::new()
            .url(format!("http://{}:{}", host, api_config.port))
            .description(Some("Local"))
            .build(),
    ]);

    let state = APIAppState {
        auth_token,
        rate_map: Arc::default(),
        core_cmd_tx,
        db,
        network,
        pending_nearby,
        event_tx,
        app_data_dir,
    };

    let protected = Router::new()
        .route("/ws", any(ws_handler))
        //
        .route("/self/user_id", get(api_self_get_user_id))
        .route("/self/profile", get(api_self_get_profile))
        .route("/self/profile", post(api_self_post_profile))
        .route("/self/peer_id", get(api_self_get_peer_id))
        .route("/self/peer_id", delete(api_self_rotate_peer_id))
        //
        .route("/otp/generate", post(api_otp_generate))
        .route("/otp/fetch", post(api_otp_fetch))
        //
        .route("/ltc/export", post(api_ltc_export))
        .route("/ltc/import", post(api_ltc_import))
        //
        .route("/nearby/start", post(api_nearby_start))
        .route("/nearby/stop", post(api_nearby_stop))
        .route("/nearby", get(api_nearby_get))
        .route(
            "/nearby/{peer_id}/connect/{method}",
            post(api_nearby_connect),
        )
        .route("/nearby/{peer_id}/accept", post(api_nearby_accept))
        .route("/nearby/{peer_id}/decline", post(api_nearby_decline))
        //
        .route("/contacts", get(api_contacts))
        .route(
            "/contact/{contact_id}/security_code",
            get(api_contact_security_code),
        )
        .route(
            "/contact/{contact_id}/security_code",
            post(api_contact_security_code_confirm),
        )
        .route(
            "/contact/{contact_id}/profile",
            post(api_contact_share_profile),
        )
        .route("/contact/{contact_id}", get(api_contact_get))
        .route("/contact/{contact_id}", delete(api_contact_remove))
        .route("/contacts/blocked", get(api_contact_blocked_list))
        .route("/contact/{contact_id}/block", post(api_contact_block))
        .route("/contact/{contact_id}/unblock", post(api_contact_unblock))
        //
        .route("/typing/{contact_id}", post(api_typing))
        .route("/messages/{contact_id}", post(api_messages_send))
        .route("/messages/{contact_id}", get(api_messages_get))
        .route(
            "/messages/{contact_id}/{message_id}/local",
            delete(api_message_delete_local),
        )
        .route(
            "/messages/{contact_id}/{message_id}",
            delete(api_message_delete),
        )
        .route(
            "/messages/{contact_id}/{message_id}/pin",
            post(api_message_pin),
        )
        .route(
            "/messages/{contact_id}/{message_id}/unpin",
            post(api_message_unpin),
        )
        .route(
            "/messages/{contact_id}/{message_id}",
            patch(api_message_edit),
        )
        .route(
            "/messages/{contact_id}/{message_id}/reactions/{emoji}",
            post(api_message_reaction_add),
        )
        .route(
            "/messages/{contact_id}/{message_id}/reactions/{emoji}",
            delete(api_message_reaction_remove),
        )
        .route("/files/{contact_id}", post(api_files_send))
        .route("/files/{contact_id}/{offer_id}", post(api_files_accept))
        //
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ));

    let app = Router::new()
        .merge(protected)
        .merge(Scalar::with_url("/", openapi))
        .layer(CorsLayer::new())
        .with_state(state)
        .into_make_service_with_connect_info::<SocketAddr>();

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, api_config.port))
        .await
        .ok_kursal(KursalError::Network)?;

    axum::serve(listener, app)
        .await
        .ok_kursal(KursalError::Network)?;

    Ok(())
}
