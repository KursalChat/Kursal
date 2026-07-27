use crate::Result;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum HangupReason {
    Declined,
    Busy,
    Hangup,
    Timeout,
    Error,
}

#[derive(Serialize, Deserialize)]
pub struct OfferPayload {
    pub random: [u8; 32],
    pub sample_rate: u32,
    pub channels: u8,
}

#[derive(Serialize, Deserialize)]
pub struct AnswerPayload {
    pub random: [u8; 32],
}

#[derive(Serialize, Deserialize)]
pub struct HangupPayload {
    pub reason: HangupReason,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum VideoStopReason {
    Toggle,
    Unsupported,
    Error,
}

#[derive(Serialize, Deserialize)]
pub struct VideoStartPayload {
    pub codec: String,
    pub width: u16,
    pub height: u16,
}

#[derive(Serialize, Deserialize)]
pub struct VideoStopPayload {
    pub reason: VideoStopReason,
}

#[derive(Serialize, Deserialize)]
pub struct VoiceStatePayload {
    pub muted: bool,
    pub deafened: bool,
}

pub fn encode<T: Serialize>(v: &T) -> Result<Vec<u8>> {
    bincode::serialize(v).map_err(Into::into)
}

pub fn decode<T: DeserializeOwned>(b: &[u8]) -> Result<T> {
    bincode::deserialize(b).map_err(Into::into)
}
