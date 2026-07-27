use crate::MapKursalResult;
use crate::{KursalError, Result, crypto::derive_key};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use rand::{TryRngCore, rngs::OsRng};

const NONCE_LEN: usize = 24;
const MIN_CIPHERTEXT_LEN: usize = NONCE_LEN + 16;

pub fn derive_stream_key(my_random: [u8; 32], their_random: [u8; 32]) -> Result<[u8; 32]> {
    let combined = [my_random, their_random].concat();

    derive_key(&combined, b"kursal-stream-key")
}

pub fn stream_encrypt(key: &[u8; 32], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).ok_kursal(KursalError::Crypto)?;

    let mut nonce = [0u8; NONCE_LEN];
    OsRng
        .try_fill_bytes(&mut nonce)
        .ok_kursal(KursalError::Crypto)?;

    let encrypted = cipher
        .encrypt(&XNonce::from(nonce), plaintext)
        .ok_kursal(KursalError::Crypto)?;

    let mut output = Vec::with_capacity(NONCE_LEN + encrypted.len());
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&encrypted);

    Ok(output)
}

pub fn stream_decrypt(key: &[u8; 32], ciphertext: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.len() < MIN_CIPHERTEXT_LEN {
        return Err(KursalError::Crypto(format!(
            "Ciphertext must be at least {MIN_CIPHERTEXT_LEN} bytes"
        )));
    }

    let (nonce, cipherinput) = ciphertext.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce.try_into().ok_kursal(KursalError::Crypto)?;

    let cipher = XChaCha20Poly1305::new_from_slice(key).ok_kursal(KursalError::Crypto)?;

    let decrypted = cipher
        .decrypt(&XNonce::from(nonce), cipherinput)
        .ok_kursal(KursalError::Crypto)?;

    Ok(decrypted)
}

pub fn stream_encrypt_aad(key: &[u8; 32], plaintext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).ok_kursal(KursalError::Crypto)?;

    let mut nonce = [0u8; NONCE_LEN];
    OsRng
        .try_fill_bytes(&mut nonce)
        .ok_kursal(KursalError::Crypto)?;

    let encrypted = cipher
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .ok_kursal(KursalError::Crypto)?;

    let mut output = Vec::with_capacity(NONCE_LEN + encrypted.len());
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&encrypted);

    Ok(output)
}

pub fn stream_decrypt_aad(key: &[u8; 32], ciphertext: &[u8], aad: &[u8]) -> Result<Vec<u8>> {
    if ciphertext.len() < MIN_CIPHERTEXT_LEN {
        return Err(KursalError::Crypto(format!(
            "Ciphertext must be at least {MIN_CIPHERTEXT_LEN} bytes"
        )));
    }

    let (nonce, cipherinput) = ciphertext.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce.try_into().ok_kursal(KursalError::Crypto)?;

    let cipher = XChaCha20Poly1305::new_from_slice(key).ok_kursal(KursalError::Crypto)?;

    let decrypted = cipher
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: cipherinput,
                aad,
            },
        )
        .ok_kursal(KursalError::Crypto)?;

    Ok(decrypted)
}
