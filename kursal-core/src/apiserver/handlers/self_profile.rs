use crate::{
    api::cmd_wrapper,
    apiserver::{
        APIAppState, Result,
        types::{APIError, APISelfProfile},
    },
};
use axum::{Json, extract::State};

#[utoipa::path(
    delete,
    path = "/self/peer_id",
    tag = "Self",
    responses(
        (status = 200, description = "Peer ID rotated"),
        (status = 401, description = "Unauthorized", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub(crate) async fn api_self_rotate_peer_id(State(state): State<APIAppState>) -> Result<()> {
    cmd_wrapper::rotate_peer_id(state).await.map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/self/peer_id",
    tag = "Self",
    responses(
        (status = 200, description = "Peer ID", body = String)
    )
)]
pub(crate) async fn api_self_get_peer_id(State(state): State<APIAppState>) -> Result<String> {
    cmd_wrapper::get_local_peer_id(state)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/self/user_id",
    tag = "Self",
    responses(
        (status = 200, description = "User ID", body = String)
    )
)]
pub(crate) async fn api_self_get_user_id(State(state): State<APIAppState>) -> Result<String> {
    cmd_wrapper::get_local_user_id_hex(state)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/self/profile",
    tag = "Self",
    responses(
        (status = 200, description = "User profile", body = APISelfProfile)
    )
)]
pub(crate) async fn api_self_get_profile(State(state): State<APIAppState>) -> Json<APISelfProfile> {
    let (username, avatar) = cmd_wrapper::get_local_user_profile(state).await;
    Json(APISelfProfile { username, avatar })
}

#[utoipa::path(
    post,
    path = "/self/profile",
    tag = "Self",
    request_body = APISelfProfile,
    responses(
        (status = 200, description = "Profile updated")
    )
)]
pub(crate) async fn api_self_post_profile(
    State(state): State<APIAppState>,
    Json(APISelfProfile { username, avatar }): Json<APISelfProfile>,
) -> Result<()> {
    cmd_wrapper::broadcast_profile(state, username, avatar)
        .await
        .map_err(Into::into)
}
