use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct APIError {
    pub(crate) message: String,
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
