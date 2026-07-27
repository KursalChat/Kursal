use crate::MapKursalResult;
use crate::{KursalError, Result};
use pqcrypto_dilithium::dilithium5;
use pqcrypto_traits::sign::{DetachedSignature, PublicKey, SecretKey, VerificationError};
use zeroize::Zeroize;

pub fn dilithium_sign(mut secret_key_bytes: Vec<u8>, message: &[u8]) -> Result<Vec<u8>> {
    let sig = dilithium5::detached_sign(
        message,
        &dilithium5::SecretKey::from_bytes(&secret_key_bytes).ok_kursal(KursalError::Crypto)?,
    );

    secret_key_bytes.zeroize();

    Ok(sig.as_bytes().to_vec())
}

pub fn dilithium_verify(public_key_bytes: &[u8], message: &[u8], signature: &[u8]) -> Result<bool> {
    match dilithium5::verify_detached_signature(
        &dilithium5::DetachedSignature::from_bytes(signature).ok_kursal(KursalError::Crypto)?,
        message,
        &dilithium5::PublicKey::from_bytes(public_key_bytes).ok_kursal(KursalError::Crypto)?,
    ) {
        Ok(_) => Ok(true),
        Err(VerificationError::InvalidSignature) => Ok(false),
        Err(err) => Err(KursalError::Crypto(err.to_string())),
    }
}
