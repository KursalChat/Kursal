use futures::channel::mpsc::SendError;

use crate::error::{Error, ErrorType};

impl From<windows::core::Error> for Error {
    fn from(value: windows::core::Error) -> Self {
        Error::new(
            format!("windows::core::Error: {:?}", value.code()),
            format!("{:?}", value),
            ErrorType::Windows,
        )
    }
}

impl From<SendError> for Error {
    fn from(value: SendError) -> Self {
        Error::new(
            "futures::channel::mpsc::SendError",
            format!("{:?}", value).as_str(),
            ErrorType::Windows,
        )
    }
}
