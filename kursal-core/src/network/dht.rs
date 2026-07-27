use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    network::kademlia::{KAD_LONG_MAX_AGE, KAD_MAX_AGE, KAD_MAX_CLOCK_SKEW, KAD_MAX_PAYLOAD},
    storage::get_timestamp_secs,
};
use argon2::{Argon2, Params};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

const POW_SALT: &[u8; 16] = b"kursal-dht-pow01";

pub const POW_PREFILTER_TARGET: [u8; 32] = [
    0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
];

pub const DHT_TARGET: [u8; 32] = [
    0x0F, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
];

pub const DHT_LONG_TARGET: [u8; 32] = [
    0x07, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
];

static POW_ARGON2: LazyLock<Option<Argon2<'static>>> = LazyLock::new(|| {
    Params::new(8 * 1024, 1, 1, Some(32))
        .ok()
        .map(|params| Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params))
});

#[derive(Serialize, Deserialize)]
pub struct DHTRecord {
    pub key: Vec<u8>,
    pub value: Vec<u8>,
    pub proof: u128,
    pub timestamp: u64,
    pub is_long: bool,
}

impl DHTRecord {
    pub async fn new(key: Vec<u8>, value: Vec<u8>, timestamp: u64, is_long: bool) -> Result<Self> {
        let payload = record_payload(&key, &value, timestamp, is_long);

        let proof = mine_pow_async(
            if is_long { DHT_LONG_TARGET } else { DHT_TARGET },
            payload,
            [0u8; 32],
        )
        .await?;

        Ok(Self {
            key,
            value,
            proof,
            timestamp,
            is_long,
        })
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }

    pub fn raw_parse(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }

    pub fn deserialize(key: &[u8], bytes: &[u8]) -> Result<Vec<u8>> {
        let record = DHTRecord::is_valid(key, bytes)?;
        Ok(record.value)
    }

    pub fn is_valid(key: &[u8], bytes: &[u8]) -> Result<Self> {
        if bytes.len() > KAD_MAX_PAYLOAD {
            return Err(KursalError::Crypto(format!(
                "payload too large: {} bytes (max {} bytes)",
                bytes.len(),
                KAD_MAX_PAYLOAD
            )));
        }

        let record: Self = bincode::deserialize(bytes)?;

        if record.key != key {
            return Err(KursalError::Crypto(
                "Record does not have the correct key".to_string(),
            ));
        }

        let now = get_timestamp_secs()?;

        if record.timestamp
            < now.saturating_sub(if record.is_long {
                KAD_LONG_MAX_AGE
            } else {
                KAD_MAX_AGE
            })
        {
            return Err(KursalError::Crypto(
                "Timestamp in the record too old".to_string(),
            ));
        }

        if record.timestamp > now.saturating_add(KAD_MAX_CLOCK_SKEW) {
            return Err(KursalError::Crypto(
                "Timestamp in the record is in the future".to_string(),
            ));
        }

        let payload = record_payload(&record.key, &record.value, record.timestamp, record.is_long);

        if !check_pow(
            if record.is_long {
                &DHT_LONG_TARGET
            } else {
                &DHT_TARGET
            },
            &payload,
            record.proof,
            &[0u8; 32],
        ) {
            return Err(KursalError::Crypto("Proof of work is invalid".to_string()));
        }

        Ok(record)
    }
}

//

fn record_payload(key: &[u8], value: &[u8], timestamp: u64, is_long: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(key.len() + value.len() + 9);
    out.extend_from_slice(key);
    out.extend_from_slice(value);
    out.extend_from_slice(&timestamp.to_le_bytes());
    out.push(is_long as u8);
    out
}

pub async fn mine_pow_async(target: [u8; 32], message: Vec<u8>, tag: [u8; 32]) -> Result<u128> {
    tokio::task::spawn_blocking(move || mine_pow(&target, &message, &tag))
        .await
        .ok_kursal(KursalError::Crypto)?
}

pub fn mine_pow(target: &[u8; 32], message: &[u8], tag: &[u8; 32]) -> Result<u128> {
    let threads = rayon::current_num_threads() as u128;

    (0..threads)
        .into_par_iter()
        .find_map_any(|thread_id| {
            let mut nonce = thread_id;
            loop {
                if check_pow(target, message, nonce, tag) {
                    return Some(nonce);
                }
                nonce = nonce.checked_add(threads)?;
            }
        })
        .ok_or_else(|| KursalError::Crypto("Failed to find valid POW nonce".to_string()))
}

pub fn check_pow(target: &[u8; 32], message: &[u8], nonce: u128, tag: &[u8; 32]) -> bool {
    let mut input = Vec::with_capacity(tag.len() + message.len() + 16);
    input.extend_from_slice(tag);
    input.extend_from_slice(message);
    input.extend_from_slice(&nonce.to_le_bytes());

    let sha: [u8; 32] = Sha256::digest(&input).into();
    if sha > POW_PREFILTER_TARGET {
        return false;
    }

    let Some(argon2) = POW_ARGON2.as_ref() else {
        return false;
    };
    let mut hash = [0u8; 32];
    if argon2
        .hash_password_into(&input, POW_SALT, &mut hash)
        .is_err()
    {
        return false;
    }
    &hash <= target
}
