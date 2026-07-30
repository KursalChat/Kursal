use crate::{
    api::cmd_wrapper,
    apiserver::{
        APIAppState, Result,
        types::{APIError, APISelfProfile},
    },
    dto::ContactResponse,
};
use axum::{
    Json,
    extract::{Path, State},
};

#[utoipa::path(
    get,
    path = "/contacts",
    tag = "Contacts",
    responses(
        (status = 200, description = "Got contacts", body = Vec<ContactResponse>)
    )
)]
pub(crate) async fn api_contacts(
    State(state): State<APIAppState>,
) -> Result<Json<Vec<ContactResponse>>> {
    cmd_wrapper::get_contacts(state)
        .await
        .map(Json)
        .map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/contact/{contact_id}",
    tag = "Contacts",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
    ),
    responses(
        (status = 200, description = "Fetched contact", body = Option<ContactResponse>)
    )
)]
pub(crate) async fn api_contact_get(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<Json<Option<ContactResponse>>> {
    cmd_wrapper::get_contact(state, contact_id)
        .await
        .map(Json)
        .map_err(Into::into)
}

#[utoipa::path(
    delete,
    path = "/contact/{contact_id}",
    tag = "Contacts",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
    ),
    responses(
        (status = 200, description = "Removed contact")
    )
)]
pub(crate) async fn api_contact_remove(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::remove_contact(state, contact_id)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/contact/{contact_id}/security_code",
    tag = "Contacts",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
    ),
    responses(
        (status = 200, description = "Got security code")
    )
)]
pub(crate) async fn api_contact_security_code(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<String> {
    cmd_wrapper::get_security_code(state, contact_id)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/contact/{contact_id}/security_code",
    tag = "Contacts",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
    ),
    responses(
        (status = 200, description = "Validated security code")
    )
)]
pub(crate) async fn api_contact_security_code_confirm(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::confirm_security_code(state, contact_id)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/contact/{contact_id}/block",
    tag = "Contacts",
    params(
        ("contact_id" = String, Path, description = "Blocked contact")
    ),
    responses(
        (status = 200, description = "Blocked contact"),
        (status = 401, description = "Unauthorized", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub(crate) async fn api_contact_block(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::set_contact_blocked(state, contact_id, true)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/contact/{contact_id}/unblock",
    tag = "Contacts",
    params(
        ("contact_id" = String, Path, description = "Unblocked contact")
    ),
    responses(
        (status = 200, description = "Unblocked contact"),
        (status = 401, description = "Unauthorized", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub(crate) async fn api_contact_unblock(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::set_contact_blocked(state, contact_id, false)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    get,
    path = "/contacts/blocked",
    tag = "Contacts",
    responses(
        (status = 200, description = "Listed blocked contacts", body = Vec<ContactResponse>)
    )
)]
pub(crate) async fn api_contact_blocked_list(
    State(state): State<APIAppState>,
) -> Result<Json<Vec<ContactResponse>>> {
    cmd_wrapper::get_blocked_contacts(state)
        .await
        .map(Json)
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/contact/{contact_id}/profile",
    tag = "Contacts",
    params(
        ("contact_id" = String, Path, description = "Contact ID")
    ),
    request_body = APISelfProfile,
    responses(
        (status = 200, description = "Shared profile")
    )
)]
pub(crate) async fn api_contact_share_profile(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
    Json(APISelfProfile { username, avatar }): Json<APISelfProfile>,
) -> Result<()> {
    cmd_wrapper::share_profile(state, username, avatar, contact_id)
        .await
        .map_err(Into::into)
}
