use crate::{KursalError, Result};

pub fn encode_frame(seq: u64, cipher: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + cipher.len());
    out.extend_from_slice(&seq.to_be_bytes());
    out.extend_from_slice(cipher);
    out
}

pub fn decode_frame(buf: &[u8]) -> Result<(u64, &[u8])> {
    if buf.len() < 8 {
        return Err(KursalError::Network("call frame too short".into()));
    }
    let seq = u64::from_be_bytes(buf[..8].try_into().unwrap());
    Ok((seq, &buf[8..]))
}
