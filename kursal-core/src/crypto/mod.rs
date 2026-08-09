//! - PQXDH is done via `AliceSignalProtocolParameters` + `initialize_alice_session_record` and `BobSignalProtocolParameters` + `initialize_bob_session_record`.
//!
//! - Double Ratchet done via `message_encrypt` / `message_decrypt` with `&mut Database` as store.
//! - `PreKeyBundle` + `process_prekey_bundle` for the standard prekey flow.

use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    identity::generators::{generate_kyber_prekey, generate_prekey, generate_signed_prekey},
    storage::{SharedDatabase, get_local_address},
};
use hkdf::Hkdf;
use libsignal_protocol::{
    DeviceId, GenericSignedPreKey, IdentityKey, IdentityKeyStore, KyberPreKeyId, PreKeyBundle,
    PreKeyId, ProtocolAddress, PublicKey as SignalPublicKey, SignedPreKeyId, kem,
    process_prekey_bundle,
};
use rand::{TryRngCore, rngs::OsRng};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::time::SystemTime;
use zeroize::Zeroizing;

pub mod dilithium;
pub mod messages;
pub mod stream;

pub const DEVICE_ID: DeviceId = match DeviceId::new(1) {
    Ok(id) => id,
    Err(_) => unreachable!(),
};

pub struct PreKeyBundleData {
    pub registration_id: u32,
    // pub device_id: u32,
    pub pre_key_id: Option<PreKeyId>,
    pub pre_key_public: Option<SignalPublicKey>,
    pub signed_pre_key_id: SignedPreKeyId,
    pub signed_pre_key_public: SignalPublicKey,
    pub signed_pre_key_signature: Vec<u8>,
    pub identity_key: IdentityKey,
    pub kyber_pre_key_id: KyberPreKeyId,
    pub kyber_pre_key_public: kem::PublicKey,
    pub kyber_pre_key_signature: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct PreKeyBundleBytes {
    pub registration_id: u32,
    pub pre_key_id: Option<u32>,
    pub pre_key_public: Option<Vec<u8>>,
    pub signed_pre_key_id: u32,
    pub signed_pre_key_public: Vec<u8>,
    pub signed_pre_key_signature: Vec<u8>,
    pub identity_key: Vec<u8>,
    pub kyber_pre_key_id: u32,
    pub kyber_pre_key_public: Vec<u8>,
    pub kyber_pre_key_signature: Vec<u8>,
}

impl PreKeyBundleData {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let bytes = PreKeyBundleBytes {
            registration_id: self.registration_id,
            pre_key_id: self.pre_key_id.map(|e| e.into()),
            pre_key_public: self.pre_key_public.as_ref().map(|k| k.serialize().to_vec()),
            signed_pre_key_id: self.signed_pre_key_id.into(),
            signed_pre_key_public: self.signed_pre_key_public.serialize().to_vec(),
            signed_pre_key_signature: self.signed_pre_key_signature.clone(),
            identity_key: self.identity_key.public_key().serialize().to_vec(),
            kyber_pre_key_id: self.kyber_pre_key_id.into(),
            kyber_pre_key_public: self.kyber_pre_key_public.serialize().to_vec(),
            kyber_pre_key_signature: self.kyber_pre_key_signature.clone(),
        };

        bincode::serialize(&bytes).map_err(Into::into)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        let bundle =
            bincode::deserialize::<PreKeyBundleBytes>(bytes).ok_kursal(KursalError::Storage)?;

        Ok(PreKeyBundleData {
            registration_id: bundle.registration_id,
            pre_key_id: bundle.pre_key_id.map(|e| e.into()),
            pre_key_public: bundle
                .pre_key_public
                .as_deref()
                .map(|b| SignalPublicKey::deserialize(b).ok_kursal(KursalError::Crypto))
                .transpose()?,
            signed_pre_key_id: bundle.signed_pre_key_id.into(),
            signed_pre_key_public: SignalPublicKey::deserialize(&bundle.signed_pre_key_public)
                .ok_kursal(KursalError::Crypto)?,
            signed_pre_key_signature: bundle.signed_pre_key_signature,
            identity_key: IdentityKey::decode(&bundle.identity_key)
                .ok_kursal(KursalError::Crypto)?,
            kyber_pre_key_id: bundle.kyber_pre_key_id.into(),
            kyber_pre_key_public: kem::PublicKey::deserialize(&bundle.kyber_pre_key_public)
                .ok_kursal(KursalError::Crypto)?,
            kyber_pre_key_signature: bundle.kyber_pre_key_signature,
        })
    }

    async fn builder_build_pre_key_bundle(
        db: SharedDatabase,
        with_prekey: bool,
        last_resort_kyber: bool,
    ) -> Result<PreKeyBundleData> {
        let registration_id = db
            .get_local_registration_id()
            .await
            .ok_kursal(KursalError::Storage)?;

        let mut rng = OsRng.unwrap_err();

        let (prekey_id, prekey) = {
            if with_prekey {
                let (prekey_id, prekey_record) = generate_prekey(db.clone(), &mut rng).await?;

                let prekey_public = prekey_record.public_key().ok_kursal(KursalError::Storage)?;

                (Some(prekey_id), Some(prekey_public))
            } else {
                (None, None)
            }
        };

        let identity_keypair = db
            .get_identity_key_pair()
            .await
            .ok_kursal(KursalError::Storage)?;

        let (signed_prekey_id, signed_prekey) =
            generate_signed_prekey(db.clone(), &identity_keypair, &mut rng).await?;

        let (kyber_prekey_id, kyber_prekey) =
            generate_kyber_prekey(db.clone(), &identity_keypair, &mut rng, last_resort_kyber)
                .await?;

        Ok(PreKeyBundleData {
            registration_id,
            pre_key_id: prekey_id,
            pre_key_public: prekey,
            signed_pre_key_id: signed_prekey_id,
            signed_pre_key_public: signed_prekey.public_key().ok_kursal(KursalError::Storage)?,
            signed_pre_key_signature: signed_prekey.signature().ok_kursal(KursalError::Storage)?,
            identity_key: *identity_keypair.identity_key(),
            kyber_pre_key_id: kyber_prekey_id,
            kyber_pre_key_public: kyber_prekey.public_key().ok_kursal(KursalError::Storage)?,
            kyber_pre_key_signature: kyber_prekey.signature().ok_kursal(KursalError::Storage)?,
        })
    }

    pub async fn build_pre_key_bundle(db: SharedDatabase) -> Result<PreKeyBundleData> {
        PreKeyBundleData::builder_build_pre_key_bundle(db, true, false).await
    }

    pub async fn build_pre_key_bundle_noprekey(db: SharedDatabase) -> Result<PreKeyBundleData> {
        PreKeyBundleData::builder_build_pre_key_bundle(db, false, true).await
    }
}

pub async fn session_initiate(
    db: SharedDatabase,
    remote: PreKeyBundleData,
    remote_address: &ProtocolAddress,
) -> Result<()> {
    let bundle = PreKeyBundle::new(
        remote.registration_id,
        DEVICE_ID,
        remote.pre_key_id.zip(remote.pre_key_public),
        remote.signed_pre_key_id,
        remote.signed_pre_key_public,
        remote.signed_pre_key_signature,
        remote.kyber_pre_key_id,
        remote.kyber_pre_key_public,
        remote.kyber_pre_key_signature,
        remote.identity_key,
    )
    .ok_kursal(KursalError::Storage)?;

    let local_address = get_local_address(&*db.0.lock().await)?;

    let mut rng = OsRng.unwrap_err();
    process_prekey_bundle(
        remote_address,
        &local_address,
        &mut db.clone(),
        &mut db.clone(),
        &bundle,
        SystemTime::now(),
        &mut rng,
    )
    .await
    .ok_kursal(KursalError::Crypto)?;

    Ok(())
}

pub fn derive_key(secret: &[u8], derive_key: &[u8]) -> Result<[u8; 32]> {
    let hk = Hkdf::<Sha256>::new(None, secret);
    let mut key = [0u8; 32];

    hk.expand(derive_key, &mut key)
        .ok_kursal(KursalError::Crypto)?;

    Ok(key)
}

pub fn derive_key_salt(secret: &[u8], salt: &[u8], derive_key: &[u8]) -> Result<[u8; 32]> {
    let hk = Hkdf::<Sha256>::new(Some(salt), secret);
    let mut key = [0u8; 32];

    hk.expand(derive_key, &mut key)
        .ok_kursal(KursalError::Crypto)?;

    Ok(key)
}

pub fn offline_ratchet_step(chain: &[u8; 32]) -> Result<([u8; 32], [u8; 32])> {
    let mk = derive_key(chain, b"kursal-offline-msg")?;
    let next = derive_key(chain, b"kursal-offline-next")?;
    Ok((mk, next))
}

pub fn offline_tag(mk: &[u8; 32]) -> Result<[u8; 32]> {
    derive_key(mk, b"kursal-offline-tag")
}

pub fn offline_wrapper(mk: &[u8; 32]) -> Result<[u8; 32]> {
    derive_key(mk, b"kursal-offline-wrap")
}

pub fn offline_at(chain: &[u8; 32], steps: u64) -> Result<([u8; 32], [u8; 32])> {
    let mut c = Zeroizing::new(*chain);
    for _ in 0..steps {
        *c = derive_key(&*c, b"kursal-offline-next")?;
    }
    let (mk, _) = offline_ratchet_step(&c)?;
    let mk = Zeroizing::new(mk);
    Ok((offline_tag(&mk)?, offline_wrapper(&mk)?))
}

pub fn mailbox_kem_encapsulate(public_key_bytes: &[u8]) -> Result<(Zeroizing<Vec<u8>>, Vec<u8>)> {
    let public_key =
        kem::PublicKey::deserialize(public_key_bytes).ok_kursal(KursalError::Crypto)?;
    let mut rng = OsRng.unwrap_err();
    let (shared_secret, ciphertext) = public_key
        .encapsulate(&mut rng)
        .ok_kursal(KursalError::Crypto)?;
    Ok((Zeroizing::new(shared_secret.to_vec()), ciphertext.to_vec()))
}

pub fn mailbox_kem_decapsulate(
    secret_key: &kem::SecretKey,
    ciphertext: &[u8],
) -> Result<Zeroizing<Vec<u8>>> {
    let ciphertext: Box<[u8]> = ciphertext.into();
    let shared_secret = secret_key
        .decapsulate(&ciphertext)
        .ok_kursal(KursalError::Crypto)?;
    Ok(Zeroizing::new(shared_secret.to_vec()))
}
