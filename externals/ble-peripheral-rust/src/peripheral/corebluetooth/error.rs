use crate::error::{Error, ErrorType};
use tokio::sync::{mpsc, oneshot};

impl<T> From<mpsc::error::SendError<T>> for Error {
    fn from(err: mpsc::error::SendError<T>) -> Self {
        Error::from_string(err.to_string(), ErrorType::ChannelError)
    }
}

impl From<oneshot::error::RecvError> for Error {
    fn from(err: oneshot::error::RecvError) -> Self {
        Error::from_string(err.to_string(), ErrorType::ChannelError)
    }
}
