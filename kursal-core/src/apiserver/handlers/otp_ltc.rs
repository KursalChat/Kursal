use crate::{
    api::cmd_wrapper,
    apiserver::{APIAppState, Result, types::APIError},
    dto::{ContactResponse, OtpResponse},
};
use axum::{Json, body::Bytes, extract::State};

#[utoipa::path(
    post,
    path = "/otp/generate",
    tag = "OTP",
    responses(
        (status = 200, description = "Generated OTP", body = OtpResponse),
        (status = 401, description = "Unauthorized", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub(crate) async fn api_otp_generate(
    State(state): State<APIAppState>,
) -> Result<Json<OtpResponse>> {
    let otp = cmd_wrapper::generate_otp().await?;
    cmd_wrapper::publish_otp(state, otp.otp.clone()).await?;

    Ok(Json(otp))
}

#[utoipa::path(
    post,
    path = "/otp/fetch",
    tag = "OTP",
    request_body = OtpResponse,
    responses(
        (status = 200, description = "Fetched OTP", body = ContactResponse),
        (status = 401, description = "Unauthorized", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub(crate) async fn api_otp_fetch(
    State(state): State<APIAppState>,
    Json(OtpResponse { otp }): Json<OtpResponse>,
) -> Result<Json<ContactResponse>> {
    cmd_wrapper::fetch_otp(state, otp)
        .await
        .map(Json)
        .map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/ltc/export",
    tag = "LTC",
    responses(
        (status = 200, description = "Exported LTC", body = Vec<u8>, content_type = "application/octet-stream"),
        (status = 401, description = "Unauthorized", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub(crate) async fn api_ltc_export(State(state): State<APIAppState>) -> Result<Vec<u8>> {
    cmd_wrapper::export_ltc(state).await.map_err(Into::into)
}

#[utoipa::path(
    post,
    path = "/ltc/import",
    tag = "LTC",
    request_body(
        content = Vec<u8>,
        content_type = "application/octet-stream"
    ),
    responses(
        (status = 200, description = "Imported LTC", body = ContactResponse),
        (status = 400, description = "Invalid LTC payload", body = APIError),
        (status = 401, description = "Unauthorized", body = APIError),
        (status = 500, description = "Internal server error", body = APIError)
    )
)]
pub(crate) async fn api_ltc_import(
    State(state): State<APIAppState>,
    bytes: Bytes,
) -> Result<Json<ContactResponse>> {
    cmd_wrapper::import_ltc(state, bytes.to_vec())
        .await
        .map(Json)
        .map_err(Into::into)
}
