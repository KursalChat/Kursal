use crate::{
    api::cmd_wrapper,
    apiserver::{APIAppState, Result, types::APIError},
};
use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub(crate) struct APIFile {
    path: String,
}
#[derive(Serialize, ToSchema)]
pub(crate) struct APIFileDetails {
    name: String,
    size: u64,
}

#[utoipa::path(
    post,
    path = "/files/{contact_id}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID")
    ),
    request_body = APIFile,
    responses(
        (status = 200, description = "Sent file", body = APIFileDetails),
        (status = 400, description = "Invalid file path", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub(crate) async fn api_files_send(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
    Json(APIFile { path }): Json<APIFile>,
) -> Result<Json<APIFileDetails>> {
    let app_data_dir = state.app_data_dir.clone();
    cmd_wrapper::send_file_offer(state, contact_id, path, app_data_dir, false)
        .await
        .map(|(name, size, _stored_path)| Json(APIFileDetails { name, size }))
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/files/{contact_id}/{offer_id}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("offer_id" = String, Path, description = "Offer ID"),
    ),
    request_body = APIFile,
    responses(
        (status = 200, description = "Accepted file"),
        (status = 400, description = "Invalid request", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub(crate) async fn api_files_accept(
    State(state): State<APIAppState>,
    Path((contact_id, offer_id)): Path<(String, String)>,
    Json(APIFile { path }): Json<APIFile>,
) -> Result<()> {
    cmd_wrapper::accept_file_offer(state, contact_id, offer_id, path)
        .await
        .map_err(Into::into)
}
