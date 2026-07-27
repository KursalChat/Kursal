use crate::{
    api::cmd_wrapper,
    apiserver::{
        APIAppState, Result,
        types::{APIError, APINearbyConnectMethod},
    },
    dto::NearbyPeerResponse,
};
use axum::{
    Json,
    extract::{Path, State},
};

#[utoipa::path(
    post,
    path = "/nearby/start",
    tag = "Nearby",
    responses(
        (status = 200, description = "Started Nearby", body = String)
    )
)]
pub(crate) async fn api_nearby_start(State(state): State<APIAppState>) -> Result<String> {
    cmd_wrapper::start_nearby(state).await.map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/nearby/stop",
    tag = "Nearby",
    responses(
        (status = 200, description = "Stopped Nearby")
    )
)]
pub(crate) async fn api_nearby_stop(State(state): State<APIAppState>) -> Result<()> {
    cmd_wrapper::stop_nearby(state).await.map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/nearby",
    tag = "Nearby",
    responses(
        (status = 200, description = "Got Nearby peers", body = Vec<NearbyPeerResponse>)
    )
)]
pub(crate) async fn api_nearby_get(
    State(state): State<APIAppState>,
) -> Result<Json<Vec<NearbyPeerResponse>>> {
    cmd_wrapper::get_nearby_peers(state)
        .await
        .map(Json)
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/nearby/{peer_id}/connect/{method}",
    tag = "Nearby",
    params(
        ("peer_id" = String, Path, description = "Peer ID"),
        ("method" = APINearbyConnectMethod, Path, description = "Connection method"),
    ),
    responses(
        (status = 200, description = "Connected"),
        (status = 400, description = "Invalid request", body = APIError),
        (status = 401, description = "Unauthorized", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub(crate) async fn api_nearby_connect(
    State(state): State<APIAppState>,
    Path((peer_id, method)): Path<(String, APINearbyConnectMethod)>,
) -> Result<()> {
    cmd_wrapper::connect_nearby(state, peer_id, method.as_str().to_string())
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/nearby/{peer_id}/accept",
    tag = "Nearby",
    params(
        ("peer_id" = String, Path, description = "Peer ID"),
    ),
    responses(
        (status = 200, description = "Accepted Nearby connection")
    )
)]
pub(crate) async fn api_nearby_accept(
    State(state): State<APIAppState>,
    Path(peer_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::accept_nearby(state, peer_id)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/nearby/{peer_id}/decline",
    tag = "Nearby",
    params(
        ("peer_id" = String, Path, description = "Peer ID"),
    ),
    responses(
        (status = 200, description = "Declined Nearby connection")
    )
)]
pub(crate) async fn api_nearby_decline(
    State(state): State<APIAppState>,
    Path(peer_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::decline_nearby(state, peer_id)
        .await
        .map_err(Into::into)
}
