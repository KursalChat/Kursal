use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    crypto::PreKeyBundleData,
    storage::{SharedDatabase, get_local_address},
};
use libsignal_protocol::{
    PreKeySignalMessage, ProtocolAddress, PublicKey as SignalPublicKey, SignalMessage,
    message_decrypt_prekey, message_decrypt_signal, message_encrypt,
};
use rand::{TryRngCore, rngs::OsRng};
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex as StdMutex};
use std::time::SystemTime;
use tokio::sync::Mutex as AsyncMutex;

static SESSION_LOCKS: LazyLock<StdMutex<HashMap<String, Arc<AsyncMutex<()>>>>> =
    LazyLock::new(|| StdMutex::new(HashMap::new()));

fn session_lock(address: &ProtocolAddress) -> Arc<AsyncMutex<()>> {
    let mut map = SESSION_LOCKS.lock().unwrap();
    map.entry(address.to_string())
        .or_insert_with(|| Arc::new(AsyncMutex::new(())))
        .clone()
}

pub struct InitialMessage {
    pub registration_id: u32,
    pub pre_key_bundle: PreKeyBundleData,
    pub base_key_public: SignalPublicKey,
    pub ciphertext: Vec<u8>,
}

pub async fn message_send(
    db: SharedDatabase,
    remote_address: &ProtocolAddress,
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    let lock = session_lock(remote_address);
    let _guard = lock.lock().await;

    let local_address = get_local_address(&*db.0.lock().await)?;

    let mut rng = OsRng.unwrap_err();
    let encrypted = message_encrypt(
        plaintext,
        remote_address,
        &local_address,
        &mut db.clone(),
        &mut db.clone(),
        SystemTime::now(),
        &mut rng,
    )
    .await
    .ok_kursal(KursalError::Crypto)?;

    Ok(encrypted.serialize().to_vec())
}

pub async fn message_receive(
    db: SharedDatabase,
    remote_address: &ProtocolAddress,
    ciphertext: &[u8],
) -> Result<Vec<u8>> {
    let lock = session_lock(remote_address);
    let _guard = lock.lock().await;

    let local_address = get_local_address(&*db.0.lock().await)?;

    if let Ok(msg) = SignalMessage::try_from(ciphertext) {
        let mut rng = OsRng.unwrap_err();
        let decrypted = message_decrypt_signal(
            &msg,
            remote_address,
            &local_address,
            &mut db.clone(),
            &mut db.clone(),
            &mut rng,
        )
        .await
        .ok_kursal(KursalError::Crypto)?;

        Ok(decrypted)
    } else if let Ok(msg) = PreKeySignalMessage::try_from(ciphertext) {
        let mut rng = OsRng.unwrap_err();
        let decrypted = message_decrypt_prekey(
            &msg,
            remote_address,
            &local_address,
            &mut db.clone(),
            &mut db.clone(),
            &mut db.clone(),
            &db.clone(),
            &mut db.clone(),
            &mut rng,
        )
        .await
        .ok_kursal(KursalError::Crypto)?;

        Ok(decrypted)
    } else {
        Err(KursalError::Crypto("Unknown message type".to_string()))
    }
}
