use crate::{
    api::cmd_wrapper,
    apiserver::{APIAppState, Result, types::APIError},
    dto::MessageResponse,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
pub(crate) struct APIMessageSend {
    text: String,
    reply_to: Option<String>,
}

#[utoipa::path(
    post,
    path = "/messages/{contact_id}",
    tag = "Messages",
    request_body = APIMessageSend,
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
    ),
    responses(
        (status = 200, description = "Sent message", body = String),
        (status = 401, description = "Unauthorized", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub(crate) async fn api_messages_send(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
    Json(APIMessageSend { text, reply_to }): Json<APIMessageSend>,
) -> Result<String> {
    cmd_wrapper::send_text(state, contact_id, text, reply_to)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/typing/{contact_id}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID")
    ),
    responses(
        (status = 200, description = "Typing indicator sent")
    )
)]
pub(crate) async fn api_typing(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
) -> Result<()> {
    cmd_wrapper::send_typing_indicator(state, contact_id)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    delete,
    path = "/messages/{contact_id}/{message_id}/local",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("message_id" = String, Path, description = "Message ID"),
    ),
    responses(
        (status = 200, description = "Deleted message locally")
    )
)]
pub(crate) async fn api_message_delete_local(
    State(state): State<APIAppState>,
    Path((contact_id, message_id)): Path<(String, String)>,
) -> Result<()> {
    cmd_wrapper::delete_local_message(state, contact_id, message_id)
        .await
        .map_err(Into::into)
}

#[utoipa::path(
    delete,
    path = "/messages/{contact_id}/{message_id}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("message_id" = String, Path, description = "Message ID"),
    ),
    responses(
        (status = 200, description = "Deleted message")
    )
)]
pub(crate) async fn api_message_delete(
    State(state): State<APIAppState>,
    Path((contact_id, message_id)): Path<(String, String)>,
) -> Result<()> {
    cmd_wrapper::delete_message_for_everyone(state, contact_id, message_id)
        .await
        .map(|_| ())
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/messages/{contact_id}/{message_id}/pin",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("message_id" = String, Path, description = "Message ID"),
    ),
    responses(
        (status = 200, description = "Pinned message")
    )
)]
pub(crate) async fn api_message_pin(
    State(state): State<APIAppState>,
    Path((contact_id, message_id)): Path<(String, String)>,
) -> Result<()> {
    cmd_wrapper::pin_message(state, contact_id, message_id, true)
        .await
        .map(|_| ())
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/messages/{contact_id}/{message_id}/unpin",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("message_id" = String, Path, description = "Message ID"),
    ),
    responses(
        (status = 200, description = "Pinned message")
    )
)]
pub(crate) async fn api_message_unpin(
    State(state): State<APIAppState>,
    Path((contact_id, message_id)): Path<(String, String)>,
) -> Result<()> {
    cmd_wrapper::pin_message(state, contact_id, message_id, false)
        .await
        .map(|_| ())
        .map_err(Into::into)
}

#[derive(Deserialize, ToSchema)]
pub(crate) struct APIMessageEdit {
    text: String,
}

#[utoipa::path(
    patch,
    path = "/messages/{contact_id}/{message_id}",
    tag = "Messages",
    request_body = APIMessageEdit,
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("message_id" = String, Path, description = "Message ID"),
    ),
    responses(
        (status = 200, description = "Edited message")
    )
)]
pub(crate) async fn api_message_edit(
    State(state): State<APIAppState>,
    Path((contact_id, message_id)): Path<(String, String)>,
    Json(APIMessageEdit { text }): Json<APIMessageEdit>,
) -> Result<()> {
    cmd_wrapper::edit_message(state, contact_id, message_id, text)
        .await
        .map(|_| ())
        .map_err(Into::into)
}

#[derive(Deserialize)]
pub(crate) struct APIMessagesGet {
    limit: usize,
    before: Option<String>,
}

#[utoipa::path(
    get,
    path = "/messages/{contact_id}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("limit" = usize, Query, description = "Message count"),
        ("before" = Option<String>, Query, description = "Before a certain message"),
    ),
    responses(
        (status = 200, description = "Got messages", body = Vec<MessageResponse>),
        (status = 401, description = "Unauthorized", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub(crate) async fn api_messages_get(
    State(state): State<APIAppState>,
    Path(contact_id): Path<String>,
    query: Query<APIMessagesGet>,
) -> Result<Json<Vec<MessageResponse>>> {
    let query: APIMessagesGet = query.0;

    cmd_wrapper::get_messages(state, contact_id, query.limit, query.before)
        .await
        .map(Json)
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/messages/{contact_id}/{message_id}/reactions/{emoji}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("message_id" = String, Path, description = "Message ID"),
        ("emoji" = String, Path, description = "Emoji"),
    ),
    responses(
        (status = 200, description = "Sent reaction")
    )
)]
pub(crate) async fn api_message_reaction_add(
    State(state): State<APIAppState>,
    Path((contact_id, message_id, emoji)): Path<(String, String, String)>,
) -> Result<()> {
    cmd_wrapper::add_reaction(state, contact_id, message_id, emoji)
        .await
        .map(|_| ())
        .map_err(Into::into)
}

#[utoipa::path(
    delete,
    path = "/messages/{contact_id}/{message_id}/reactions/{emoji}",
    tag = "Messages",
    params(
        ("contact_id" = String, Path, description = "Contact ID"),
        ("message_id" = String, Path, description = "Message ID"),
        ("emoji" = String, Path, description = "Emoji"),
    ),
    responses(
        (status = 200, description = "Removed reaction")
    )
)]
pub(crate) async fn api_message_reaction_remove(
    State(state): State<APIAppState>,
    Path((contact_id, message_id, emoji)): Path<(String, String, String)>,
) -> Result<()> {
    cmd_wrapper::remove_reaction(state, contact_id, message_id, emoji)
        .await
        .map(|_| ())
        .map_err(Into::into)
}
