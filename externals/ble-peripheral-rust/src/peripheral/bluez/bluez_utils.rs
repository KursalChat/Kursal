use crate::error::{self, Error, ErrorType};
use bluer::gatt::local::CharacteristicControl;
use uuid::Uuid;
#[derive(Debug)]
pub(crate) struct CharNotifyHandler {
    pub service_uuid: Uuid,
    pub characteristic_uuid: Uuid,
    pub control: CharacteristicControl,
}

impl From<bluer::Error> for error::Error {
    fn from(error: bluer::Error) -> Self {
        Error::from_string(error.to_string(), ErrorType::Bluez)
    }
}
