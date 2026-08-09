use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    contacts::Contact,
    crypto::{
        DEVICE_ID, PreKeyBundleData, mailbox_kem_encapsulate, session_initiate,
        stream::{stream_decrypt, stream_encrypt},
    },
    first_contact::{
        ContactResponse, WireMessage, forget_ack_waiter, make_username, register_ack_waiter,
    },
    identity::UserId,
    messaging::{enums::MessageId, offline::new_offline_state},
    network::{
        dht::DHTRecord,
        kademlia::KAD_MAX_AGE,
        swarm::{SwarmCommand, SwarmHandle, get_listen_addrs, is_peer_connected, str_to_multiaddr},
    },
    storage::{
        SharedDatabase, TABLE_SESSIONS, TABLE_SETTINGS, get_dilithium_pub, get_timestamp_secs,
    },
};
use argon2::{Argon2, ParamsBuilder};
use libp2p::PeerId;
use libsignal_protocol::{KeyPair, ProtocolAddress};
use rand::{Rng, TryRngCore, distr::Uniform, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{str::FromStr, sync::LazyLock, time::Duration};
use tokio::sync::mpsc;
use zeroize::Zeroizing;

const SALT: &[u8; 16] = b"kursal-otp-salt1";
const WORDS: &str = include_str!("otp_wordlist.txt");
const ACK_TIMEOUT_SECS: u64 = 20;
const MAX_EDITS: usize = 2;
const MAX_SUGGESTIONS: usize = 3;
const DP_ROW: usize = 16;

static WORDLIST: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| WORDS.lines().filter(|s| !s.is_empty()).collect());

pub fn generate_otp() -> Result<String> {
    let wordlist = &*WORDLIST;

    let dist = Uniform::new(0, wordlist.len()).ok_kursal(KursalError::Crypto)?;
    let mut os_rng = OsRng;
    let mut rng = os_rng.unwrap_mut();

    // 8 random words
    let result: Result<Vec<String>> = (0..8)
        .map(|_| {
            wordlist
                .get(rng.sample(dist))
                .ok_or(KursalError::Crypto(
                    "Could not generate a random word".to_string(),
                ))
                .map(|w| w.to_string())
        })
        .collect();

    Ok(result?.join(" "))
}

pub fn hash_otp(otp: &str) -> Result<[u8; 32]> {
    let argon2 = Argon2::new(
        argon2::Algorithm::Argon2id,
        argon2::Version::V0x13,
        ParamsBuilder::new()
            .m_cost(256 * 1024)
            .t_cost(2)
            .p_cost(1)
            .output_len(32)
            .build()
            .ok_kursal(KursalError::Crypto)?,
    );

    let mut output = [0u8; 32];
    argon2
        .hash_password_into(otp.as_bytes(), SALT, &mut output)
        .ok_kursal(KursalError::Crypto)?;

    Ok(output)
}

pub fn otp_to_keys(otp: &str) -> Result<([u8; 32], [u8; 32])> {
    let hash = hash_otp(otp)?;
    let dht_key = Sha256::digest(hash).into();

    Ok((hash, dht_key))
}

#[derive(Serialize, Deserialize)]
pub struct OtpPayload {
    pub payload_id: MessageId,
    pub pre_key_bundle: Vec<u8>,
    pub peer_id: String,
    pub dilithium_pub_key: Vec<u8>,
    pub relay_addresses: Vec<String>,
}

impl OtpPayload {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }
}

pub async fn build_otp_payload(
    db: SharedDatabase,
    swarm: &SwarmHandle,
    enc_key: &[u8; 32],
    payload_id: MessageId,
) -> Result<Vec<u8>> {
    let bundle_data = PreKeyBundleData::build_pre_key_bundle(db.clone()).await?;
    let prekey_id: u32 = bundle_data
        .pre_key_id
        .ok_or_else(|| KursalError::Crypto("OTP bundle missing one-time prekey".to_string()))?
        .into();
    db.0.lock()
        .await
        .raw_write(TABLE_SETTINGS, "otp_prekey_id", &prekey_id.to_be_bytes())?;
    let bundle = bundle_data.serialize()?;
    let peer_id = swarm.peer_id.to_base58();
    let dilithium_pub_key = get_dilithium_pub(&*db.0.lock().await)?;

    let payload = OtpPayload {
        payload_id,
        pre_key_bundle: bundle,
        peer_id,
        dilithium_pub_key,
        relay_addresses: get_listen_addrs(&swarm.cmd_tx).await?,
    };

    stream_encrypt(enc_key, &payload.serialize()?)
}

pub async fn publish_otp(otp: &str, db: SharedDatabase, swarm: &SwarmHandle) -> Result<()> {
    let timestamp = get_timestamp_secs()?;
    let payload_id = MessageId::new();

    let (enc_key, dht_key) = otp_to_keys(otp)?;
    let enc_key = Zeroizing::new(enc_key);
    let payload = build_otp_payload(db.clone(), swarm, &enc_key, payload_id).await?;

    let dht_record = DHTRecord::new(dht_key.to_vec(), payload, timestamp, false).await?;

    swarm
        .cmd_tx
        .send(SwarmCommand::PublishDht {
            key: dht_key.to_vec(),
            value: dht_record.serialize()?,
            expires: Some(KAD_MAX_AGE),
            reply_tx: None,
        })
        .await
        .ok_kursal(KursalError::Network)?;

    {
        let db_lock = db.0.lock().await;
        db_lock.raw_write(TABLE_SETTINGS, "otp_published_at", &timestamp.to_be_bytes())?;
        db_lock.raw_write(TABLE_SETTINGS, "otp_pending_id", &payload_id.0)?;
        db_lock.raw_write(TABLE_SETTINGS, "otp_dht_key", &dht_key)?;
        db_lock.raw_delete(TABLE_SETTINGS, "otp_consumed_id")?;
    }

    Ok(())
}

pub async fn fetch_otp(otp: &str, db: SharedDatabase, swarm: &SwarmHandle) -> Result<Contact> {
    let local_peer_id = swarm.peer_id.to_base58();
    let timestamp = get_timestamp_secs()?;
    let (enc_key, dht_key) = otp_to_keys(otp)?;
    let enc_key = Zeroizing::new(enc_key);

    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let mut attempt = 0u32;

    let (payload, bundle) = loop {
        attempt += 1;
        let (reply_tx, mut reply_rx) = mpsc::channel(16);

        swarm
            .cmd_tx
            .send(SwarmCommand::FetchDht {
                key: dht_key.to_vec(),
                reply_tx,
            })
            .await
            .ok_kursal(KursalError::Network)?;

        log::info!(
            "[otp] fetch attempt {attempt} dht_key={}",
            hex::encode(&dht_key[..8])
        );

        let found = tokio::time::timeout_at(deadline, async {
            while let Some(bytes) = reply_rx.recv().await {
                let dht_record = match DHTRecord::is_valid(&dht_key, &bytes) {
                    Ok(record) => record,
                    Err(err) => {
                        log::debug!("[otp] Ignoring invalid DHT record: {err}");
                        continue;
                    }
                };

                let decrypted = match stream_decrypt(&enc_key, &dht_record.value) {
                    Ok(decrypted) => decrypted,
                    Err(err) => {
                        log::debug!("[otp] DHT record decrypted failed: {err}");
                        continue;
                    }
                };

                let record = match OtpPayload::deserialize(&decrypted) {
                    Ok(record) => record,
                    Err(err) => {
                        log::debug!("[otp] OTP payload deserialize failed: {err}");
                        continue;
                    }
                };

                if record.peer_id == local_peer_id {
                    log::debug!("[otp] Cannot add yourself as a contact");
                    continue;
                }

                if let Ok(bundle) = PreKeyBundleData::deserialize(&record.pre_key_bundle) {
                    return Some((record, bundle));
                }
            }
            None
        })
        .await;

        match found {
            Ok(Some(result)) => break result,
            _ => {
                if tokio::time::Instant::now() >= deadline {
                    return Err(KursalError::Network("Record not found".to_string()));
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    };

    let identity_pub_key = bundle.identity_key.public_key().serialize().to_vec();
    let user_id: [u8; 32] = Sha256::digest(&identity_pub_key).into();

    let remote_address = ProtocolAddress::new(hex::encode(user_id), DEVICE_ID);
    let mailbox_kem_prekey_id: u32 = bundle.kyber_pre_key_id.into();
    let mailbox_kem_pub = bundle.kyber_pre_key_public.serialize().to_vec();
    let mailbox_opk_pub = bundle
        .pre_key_public
        .ok_or_else(|| KursalError::Crypto("OTP bundle missing one-time prekey".to_string()))?;
    session_initiate(db.clone(), bundle, &remote_address).await?;
    let (pq_secret, mailbox_kem_ct) = mailbox_kem_encapsulate(&mailbox_kem_pub)?;

    let mut rng = OsRng.unwrap_err();
    let mailbox_ephemeral = KeyPair::generate(&mut rng);
    let classical_secret = Zeroizing::new(
        mailbox_ephemeral
            .private_key
            .calculate_agreement(&mailbox_opk_pub)
            .ok_kursal(KursalError::Crypto)?,
    );
    let mailbox_ephemeral_pub = mailbox_ephemeral.public_key.serialize().to_vec();

    let my_bundle = PreKeyBundleData::build_pre_key_bundle(db.clone()).await?;
    let dilithium_pub_key = get_dilithium_pub(&*db.0.lock().await)?;

    let contact = Contact {
        user_id: UserId(user_id),
        peer_id: payload.peer_id.clone(),
        display_name: make_username(&payload.peer_id),
        avatar_bytes: None,
        identity_pub_key: identity_pub_key.clone(),
        dilithium_pub_key: payload.dilithium_pub_key.clone(),
        known_addresses: payload.relay_addresses,
        verified: false,
        profile_shared: false,
        blocked: false,
        created_at: timestamp,
        offline: new_offline_state(&db, &identity_pub_key, &classical_secret, &pq_secret).await?,
    };

    // now build bundle back
    let response = ContactResponse {
        payload_id: payload.payload_id,
        pre_key_bundle: my_bundle.serialize()?,
        peer_id: swarm.peer_id.to_base58(),
        dilithium_pub_key,
        relay_addresses: get_listen_addrs(&swarm.cmd_tx).await?,
        mailbox_kem_ct,
        mailbox_kem_prekey_id,
        mailbox_ephemeral_pub,
    };

    let wire = WireMessage::ContactResponse(response);
    let response_bytes = bincode::serialize(&wire)?;

    let publisher_peer = PeerId::from_str(&payload.peer_id).ok_kursal(KursalError::Network)?;
    let publisher_addrs = str_to_multiaddr(&contact.known_addresses)?;

    if !is_peer_connected(&swarm.cmd_tx, publisher_peer).await {
        log::info!(
            "[otp] dialing publisher {publisher_peer} via {} addr(s)",
            publisher_addrs.len()
        );
        for addr in &publisher_addrs {
            let _ = swarm.cmd_tx.send(SwarmCommand::Dial(addr.clone())).await;
        }
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        while tokio::time::Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(250)).await;
            if is_peer_connected(&swarm.cmd_tx, publisher_peer).await {
                break;
            }
        }
    }

    let connected = is_peer_connected(&swarm.cmd_tx, publisher_peer).await;
    log::info!("[otp] sending ContactResponse to {publisher_peer} connected={connected}");

    let ack_rx = register_ack_waiter(payload.payload_id);

    let sent = swarm
        .cmd_tx
        .send(SwarmCommand::SendMessage {
            peer_id: publisher_peer,
            data: response_bytes,
            addresses: publisher_addrs,
        })
        .await;

    if sent.is_err() {
        forget_ack_waiter(payload.payload_id);
        rollback_handshake(&db, &remote_address).await;
        return Err(KursalError::Network("Could not send response".to_string()));
    }

    match tokio::time::timeout(Duration::from_secs(ACK_TIMEOUT_SECS), ack_rx).await {
        Ok(Ok(Ok(()))) => {
            contact.save(&*db.0.lock().await)?;
            Ok(contact)
        }
        Ok(Ok(Err(reason))) => {
            log::warn!("[otp] publisher refused the handshake: {}", reason.as_str());
            rollback_handshake(&db, &remote_address).await;
            Err(KursalError::Identity(reason.as_str().to_string()))
        }
        _ => {
            forget_ack_waiter(payload.payload_id);
            rollback_handshake(&db, &remote_address).await;
            Err(KursalError::Network(
                "No answer from the other device".to_string(),
            ))
        }
    }
}

async fn rollback_handshake(db: &SharedDatabase, remote_address: &ProtocolAddress) {
    let db_lock = db.0.lock().await;
    if let Err(err) = db_lock.raw_delete(TABLE_SESSIONS, &remote_address.to_string()) {
        log::warn!("[otp] could not drop the half-open session: {err}");
    }
}

// None = is in wordlist
// Some(Vec) = not in + 3 closest matches
pub fn check_words(words: &[String]) -> Vec<Option<Vec<String>>> {
    words
        .iter()
        .map(|word| {
            let word = word.trim().to_lowercase();
            if word.is_empty() {
                return Some(Vec::new());
            }
            if WORDLIST.binary_search(&word.as_str()).is_ok() {
                return None;
            }
            Some(suggest(&word))
        })
        .collect()
}

fn suggest(word: &str) -> Vec<String> {
    let typed = word.as_bytes();
    let mut scored: Vec<(usize, usize, &'static str)> = Vec::new();

    for candidate in WORDLIST.iter() {
        if candidate.len().abs_diff(word.len()) > MAX_EDITS {
            continue;
        }
        let bytes = candidate.as_bytes();
        let distance = osa_distance(typed, bytes, MAX_EDITS);
        if distance <= MAX_EDITS {
            scored.push((distance, shared_prefix(typed, bytes), candidate));
        }
    }

    scored.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(b.2)));
    scored.truncate(MAX_SUGGESTIONS);
    scored.into_iter().map(|(_, _, w)| w.to_string()).collect()
}

fn shared_prefix(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

fn osa_distance(a: &[u8], b: &[u8], max: usize) -> usize {
    let n = a.len();
    let m = b.len();
    if n.abs_diff(m) > max || m + 1 > DP_ROW {
        return max + 1;
    }

    let mut rows = [[0usize; DP_ROW]; 3];
    for (j, cell) in rows[0].iter_mut().enumerate().take(m + 1) {
        *cell = j;
    }

    for i in 1..=n {
        let cur = i % 3;
        let prev = (i + 2) % 3;
        let prev2 = (i + 1) % 3;

        rows[cur][0] = i;
        let mut row_min = i;

        for j in 1..=m {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut best = (rows[prev][j] + 1)
                .min(rows[cur][j - 1] + 1)
                .min(rows[prev][j - 1] + cost);

            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best = best.min(rows[prev2][j - 2] + 1);
            }

            rows[cur][j] = best;
            row_min = row_min.min(best);
        }

        if row_min > max {
            return max + 1;
        }
    }

    rows[n % 3][m]
}
