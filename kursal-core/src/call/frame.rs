use crate::{KursalError, Result};

pub fn encode_frame(seq: u64, cipher: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + cipher.len());
    out.extend_from_slice(&seq.to_be_bytes());
    out.extend_from_slice(cipher);
    out
}

pub fn decode_frame(buf: &[u8]) -> Result<(u64, &[u8])> {
    let Some((seq, cipher)) = buf.split_first_chunk::<8>() else {
        return Err(KursalError::Network("call frame too short".into()));
    };
    Ok((u64::from_be_bytes(*seq), cipher))
}
