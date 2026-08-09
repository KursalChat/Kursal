use crate::KursalError;
use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct APIError {
    pub(crate) message: String,
    #[serde(skip)]
    pub(crate) status: StatusCode,
}

impl APIError {
    pub(crate) fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status,
        }
    }
}

impl IntoResponse for APIError {
    fn into_response(self) -> Response {
        let status = self.status;
        (status, Json(self)).into_response()
    }
}

impl From<KursalError> for APIError {
    fn from(err: KursalError) -> Self {
        let status = match &err {
            KursalError::Hex(_) | KursalError::Address(_) | KursalError::Encoding(_) => {
                StatusCode::BAD_REQUEST
            }
            KursalError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
                StatusCode::BAD_REQUEST
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        Self::new(status, err.to_string())
    }
}

#[derive(Serialize, ToSchema)]
pub(crate) struct APIEvent {
    pub(crate) event: String,
    #[schema(value_type = Object)]
    pub(crate) payload: serde_json::Value,
}

#[derive(Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub(crate) enum APINearbyConnectMethod {
    Mdns,
    Bluetooth,
}

impl APINearbyConnectMethod {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Mdns => "mdns",
            Self::Bluetooth => "bluetooth",
        }
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
pub(crate) struct APISelfProfile {
    pub(crate) username: String,
    pub(crate) avatar: Option<Vec<u8>>,
}
