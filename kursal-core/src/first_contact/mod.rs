use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    api::AppEvent,
    contacts::Contact,
    crypto::{PreKeyBundleData, mailbox_kem_decapsulate, session_initiate},
    identity::UserId,
    messaging::{enums::MessageId, offline::new_offline_state},
    network::swarm::SwarmCommand,
    storage::{SharedDatabase, TABLE_LTC_CACHE, TABLE_SETTINGS, get_timestamp_secs},
};
use libsignal_protocol::{
    DeviceId, IdentityKeyStore, KyberPreKeyId, KyberPreKeyStore, PreKeyId, PreKeyStore,
    ProtocolAddress, PublicKey,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::sync::{LazyLock, Mutex as StdMutex};
use tokio::sync::mpsc;
use zeroize::Zeroizing;

pub mod ltc;
pub mod nearby;
pub mod otp;

const FC_REPLAY_CACHE_MAX: usize = 256;

static FC_REPLAY_CACHE: LazyLock<StdMutex<VecDeque<[u8; 32]>>> =
    LazyLock::new(|| StdMutex::new(VecDeque::with_capacity(FC_REPLAY_CACHE_MAX)));

fn fc_replay_contains(hash: &[u8; 32]) -> bool {
    FC_REPLAY_CACHE.lock().unwrap().contains(hash)
}

fn fc_replay_remember(hash: [u8; 32]) {
    let mut cache = FC_REPLAY_CACHE.lock().unwrap();
    if cache.contains(&hash) {
        return;
    }
    if cache.len() >= FC_REPLAY_CACHE_MAX {
        cache.pop_front();
    }
    cache.push_back(hash);
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ContactResponse {
    pub payload_id: MessageId,
    pub pre_key_bundle: Vec<u8>,
    pub peer_id: String,
    pub dilithium_pub_key: Vec<u8>,
    pub relay_addresses: Vec<String>,
    pub mailbox_kem_ct: Vec<u8>,
    pub mailbox_kem_prekey_id: u32,
    pub mailbox_ephemeral_pub: Vec<u8>,
}

enum HandshakeKind {
    Otp,
    Ltc,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FileTransferMessage {
    pub transfer_id: [u8; 16],
    pub index: u32,
    pub data: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub enum WireMessage {
    Encrypted(Vec<u8>),               // for normal encrypted libsignal
    ContactResponse(ContactResponse), // for OTP / LTC handshake
    FileTransfer(FileTransferMessage),
    Terminate,
}

pub fn make_username(peer_id: &str) -> String {
    let hash = Sha256::digest(peer_id.as_bytes());
    let suffix = hex::encode(&hash[..3]);

    format!("Unknown #{suffix}")
}

pub async fn handle_fc_response(
    response: ContactResponse,
    db: SharedDatabase,
    cmd_tx: &mpsc::Sender<SwarmCommand>,
    event_tx: &mpsc::Sender<AppEvent>,
) -> Result<()> {
    let response_hash: [u8; 32] = Sha256::digest(bincode::serialize(&response)?).into();
    if fc_replay_contains(&response_hash) {
        log::warn!("[fc] ignoring replayed ContactResponse (seen recently)");
        return Ok(());
    }

    let now = get_timestamp_secs()?;

    let handshake = {
        let db_lock = db.0.lock().await;

        let otp_match = match db_lock.raw_read(TABLE_SETTINGS, "otp_pending_id")? {
            Some(id) if id.as_slice() == response.payload_id.0.as_slice() => {
                let published_at: u64 = db_lock
                    .raw_read(TABLE_SETTINGS, "otp_published_at")?
                    .and_then(|b| b.try_into().ok().map(u64::from_be_bytes))
                    .unwrap_or(0);
                now.saturating_sub(published_at) <= 600
            }
            _ => false,
        };

        let ltc_match = match db_lock.raw_read(TABLE_LTC_CACHE, "ltc_current_id")? {
            Some(id) if id.as_slice() == response.payload_id.0.as_slice() => db_lock
                .raw_read(TABLE_LTC_CACHE, "ltc_current_expiry")?
                .and_then(|b| b.try_into().ok().map(u64::from_be_bytes))
                .map(|expiry: u64| now <= expiry)
                .unwrap_or(false),
            _ => false,
        };

        if otp_match {
            Some(HandshakeKind::Otp)
        } else if ltc_match {
            Some(HandshakeKind::Ltc)
        } else {
            None
        }
    };

    let Some(handshake) = handshake else {
        log::warn!("Rejected incoming ContactResponse: payload_id matches no pending handshake");
        return Ok(());
    };

    let bundle = PreKeyBundleData::deserialize(&response.pre_key_bundle)?;
    let identity_key_bytes = bundle.identity_key.public_key().serialize().to_vec();

    let user_id = UserId(Sha256::digest(&identity_key_bytes).into());

    let already_exists = Contact::load(&*db.0.lock().await, &user_id)?.is_some();
    if already_exists {
        log::warn!(
            "[fc] ignoring ContactResponse for already-established contact {} (replay or duplicate)",
            hex::encode(user_id.0)
        );
        return Ok(());
    }

    let bob_address = ProtocolAddress::new(hex::encode(user_id.0), DeviceId::new(1u8).unwrap());
    session_initiate(db.clone(), bundle, &bob_address).await?;

    let pq_secret = if response.mailbox_kem_ct.is_empty() {
        Zeroizing::new(Vec::new())
    } else {
        let record = db
            .get_kyber_pre_key(KyberPreKeyId::from(response.mailbox_kem_prekey_id))
            .await
            .ok_kursal(KursalError::Crypto)?;
        let secret_key = record.secret_key().ok_kursal(KursalError::Crypto)?;
        mailbox_kem_decapsulate(&secret_key, &response.mailbox_kem_ct)?
    };

    let classical_secret = match handshake {
        HandshakeKind::Otp => {
            if response.mailbox_ephemeral_pub.is_empty() {
                log::warn!("Rejected incoming ContactResponse: missing mailbox ephemeral");
                return Ok(());
            }
            let prekey_id =
                db.0.lock()
                    .await
                    .raw_read(TABLE_SETTINGS, "otp_prekey_id")?
                    .and_then(|b| b.try_into().ok().map(u32::from_be_bytes))
                    .ok_or_else(|| KursalError::Crypto("No OTP mailbox prekey id".to_string()))?;
            let record = db
                .get_pre_key(PreKeyId::from(prekey_id))
                .await
                .ok_kursal(KursalError::Crypto)?;
            let ephemeral_pub = PublicKey::deserialize(&response.mailbox_ephemeral_pub)
                .ok_kursal(KursalError::Crypto)?;
            Zeroizing::new(
                record
                    .private_key()
                    .ok_kursal(KursalError::Crypto)?
                    .calculate_agreement(&ephemeral_pub)
                    .ok_kursal(KursalError::Crypto)?,
            )
        }
        HandshakeKind::Ltc => {
            let keypair = db
                .get_identity_key_pair()
                .await
                .ok_kursal(KursalError::Crypto)?;
            let peer_pub =
                PublicKey::deserialize(&identity_key_bytes).ok_kursal(KursalError::Crypto)?;
            Zeroizing::new(
                keypair
                    .private_key()
                    .calculate_agreement(&peer_pub)
                    .ok_kursal(KursalError::Crypto)?,
            )
        }
    };

    let contact = Contact {
        user_id: user_id.clone(),
        peer_id: response.peer_id.clone(),
        display_name: make_username(&response.peer_id),
        avatar_bytes: None,
        identity_pub_key: identity_key_bytes.clone(),
        dilithium_pub_key: response.dilithium_pub_key.clone(),
        known_addresses: response.relay_addresses,
        verified: false,
        profile_shared: false,
        blocked: false,
        created_at: now,
        offline: new_offline_state(&db, &identity_key_bytes, &classical_secret, &pq_secret).await?,
    };

    {
        let db_lock = db.0.lock().await;
        contact.save(&db_lock)?;
        crate::storage::set_contact_terminated(&db_lock, &hex::encode(user_id.0), false)?;
        if matches!(handshake, HandshakeKind::Otp) {
            db_lock.raw_delete(TABLE_SETTINGS, "otp_pending_id")?;
            db_lock.raw_delete(TABLE_SETTINGS, "otp_published_at")?;
            db_lock.raw_delete(TABLE_SETTINGS, "otp_prekey_id")?;
        }
    }

    cmd_tx
        .send(SwarmCommand::ContactAdded {
            contact: contact.clone(),
        })
        .await
        .ok_kursal(KursalError::Network)?;

    event_tx
        .send(AppEvent::ContactAdded { contact })
        .await
        .ok_kursal(KursalError::Network)?;

    fc_replay_remember(response_hash);
    Ok(())
}
