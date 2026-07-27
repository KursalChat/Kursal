use crate::Result;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub enum KursalFile {
    LtcPayload(Vec<u8>),
    Backup(Vec<u8>),
}

impl KursalFile {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }

    pub fn get_warning(&self) -> std::result::Result<String, String> {
        match self {
            KursalFile::LtcPayload(_) => Ok(
                "This file contains a long term code. Opening it will add a new contact."
                    .to_string(),
            ),
            KursalFile::Backup(_) => Err(
                "Backup files cannot be opened like this. Please go in the settings > account menu and import the backup from there."
                    .to_string(),
            ),
        }
    }
}
