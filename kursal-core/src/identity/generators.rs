use crate::MapKursalResult;
use crate::{
    KursalError, Result,
    storage::{
        Database, SharedDatabase, TABLE_IDENTITY_KEYS, TABLE_SETTINGS, get_timestamp_millis,
    },
};
use libsignal_protocol::{
    GenericSignedPreKey, IdentityKeyPair, KeyPair, KyberPreKeyId, KyberPreKeyRecord,
    KyberPreKeyStore, PreKeyId, PreKeyRecord, PreKeyStore, SignedPreKeyId, SignedPreKeyRecord,
    SignedPreKeyStore, Timestamp, kem,
};
use pqcrypto_dilithium::dilithium5;
use pqcrypto_traits::sign::{PublicKey, SecretKey};
use rand::{RngCore, TryRngCore, rand_core::UnwrapErr, rngs::OsRng};
use zeroize::Zeroizing;

pub fn generate_identity_keypair(db: &mut Database) -> Result<()> {
    let mut rng = OsRng.unwrap_err();
    let idkey = IdentityKeyPair::generate(&mut rng);

    let serialized = Zeroizing::new(idkey.serialize().to_vec());
    db.raw_write(TABLE_IDENTITY_KEYS, "local_identity", &serialized)?;

    let regid = rand::random::<u32>();
    db.raw_write(TABLE_SETTINGS, "registration_id", &regid.to_be_bytes())?;

    Ok(())
}

pub async fn generate_prekey(
    mut db: SharedDatabase,
    rng: &mut UnwrapErr<OsRng>,
) -> Result<(PreKeyId, PreKeyRecord)> {
    let keypair = KeyPair::generate(rng);

    let prekeyid = PreKeyId::from(rng.next_u32());
    let record = PreKeyRecord::new(prekeyid, &keypair);

    db.save_pre_key(prekeyid, &record)
        .await
        .ok_kursal(KursalError::Crypto)?;

    Ok((prekeyid, record))
}

pub async fn generate_signed_prekey(
    mut db: SharedDatabase,
    identity: &IdentityKeyPair,
    rng: &mut UnwrapErr<OsRng>,
) -> Result<(SignedPreKeyId, SignedPreKeyRecord)> {
    let key_pair = KeyPair::generate(rng);

    let sig = identity
        .private_key()
        .calculate_signature(&key_pair.public_key.serialize(), rng)
        .ok_kursal(KursalError::Crypto)?;

    let timestamp = get_timestamp_millis()?;
    let signed_prekey_id = SignedPreKeyId::from(rng.next_u32());

    let signed_prekey = SignedPreKeyRecord::new(
        signed_prekey_id,
        Timestamp::from_epoch_millis(timestamp),
        &key_pair,
        &sig,
    );

    db.save_signed_pre_key(signed_prekey_id, &signed_prekey)
        .await
        .ok_kursal(KursalError::Crypto)?;

    Ok((signed_prekey_id, signed_prekey))
}

pub async fn generate_kyber_prekey(
    mut db: SharedDatabase,
    identity: &IdentityKeyPair,
    rng: &mut UnwrapErr<OsRng>,
    last_resort: bool,
) -> Result<(KyberPreKeyId, KyberPreKeyRecord)> {
    let key_pair = kem::KeyPair::generate(kem::KeyType::Kyber1024, rng);

    let sig = identity
        .private_key()
        .calculate_signature(&key_pair.public_key.serialize(), rng)
        .ok_kursal(KursalError::Crypto)?;

    let timestamp = get_timestamp_millis()?;

    let kyber_prekey_id = KyberPreKeyId::from(rng.next_u32());
    let kyber_prekey = KyberPreKeyRecord::new(
        kyber_prekey_id,
        Timestamp::from_epoch_millis(timestamp),
        &key_pair,
        &sig,
    );

    db.save_kyber_pre_key(kyber_prekey_id, &kyber_prekey)
        .await
        .ok_kursal(KursalError::Crypto)?;

    if last_resort {
        db.raw_write(
            TABLE_SETTINGS,
            &format!("kyber_lastresort_{}", u32::from(kyber_prekey_id)),
            &[1u8],
        )?;
    }

    Ok((kyber_prekey_id, kyber_prekey))
}

pub fn generate_dilithium_keypair(db: &mut Database) -> Result<()> {
    let key_pair = dilithium5::keypair();

    let secret = Zeroizing::new(key_pair.1.as_bytes().to_vec());
    db.raw_write(TABLE_SETTINGS, "dilithium_secret", &secret)?;

    db.raw_write(TABLE_SETTINGS, "dilithium_public", key_pair.0.as_bytes())?;

    Ok(())
}
