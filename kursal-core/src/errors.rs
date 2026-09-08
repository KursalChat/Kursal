use thiserror::Error;

pub type Result<T> = std::result::Result<T, KursalError>;

#[derive(Error, Debug)]
pub enum KursalError {
    #[error("storage error: {0}")]
    Storage(String),
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("network error: {0}")]
    Network(String),
    #[error("identity error: {0}")]
    Identity(String),
    #[error("master key does not match this database")]
    KeyMismatch,
    #[error("not enough free space: {needed} bytes needed, {available} available")]
    InsufficientSpace { needed: u64, available: u64 },
    #[error("cannot strip metadata from this image")]
    UnstrippableImage,

    // Typed external sources: just use `?`
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    DbTransaction(#[from] redb::TransactionError),
    #[error("database error: {0}")]
    DbTable(#[from] redb::TableError),
    #[error("database error: {0}")]
    DbStorage(#[from] redb::StorageError),
    #[error("database error: {0}")]
    DbCommit(#[from] redb::CommitError),
    #[error("database error: {0}")]
    DbOpen(#[from] redb::DatabaseError),
    #[error("encoding error: {0}")]
    Encoding(#[from] bincode::Error),
    #[error("signal protocol error: {0}")]
    Signal(#[from] libsignal_protocol::SignalProtocolError),
    #[error("invalid address: {0}")]
    Address(#[from] libp2p::multiaddr::Error),
    #[error("invalid hex: {0}")]
    Hex(#[from] hex::FromHexError),
    #[error("{0}")]
    Misc(#[from] anyhow::Error),
}

pub trait MapKursalResult<T> {
    fn ok_kursal(self, variant: impl Fn(String) -> KursalError) -> Result<T>;
}

impl<T, E: std::fmt::Display> MapKursalResult<T> for std::result::Result<T, E> {
    fn ok_kursal(self, variant: impl Fn(String) -> KursalError) -> Result<T> {
        self.map_err(|e| variant(e.to_string()))
    }
}

impl From<KursalError> for String {
    fn from(val: KursalError) -> String {
        val.to_string()
    }
}
