use crate::{KursalError, Result};
use serde::{Deserialize, Serialize};
use uuid::{NoContext, Timestamp};

#[derive(Serialize, Deserialize)]
pub enum Direction {
    Sent,
    Received,
}

#[derive(Serialize, Deserialize)]
pub enum MessageStatus {
    Sending,
    Delivered,
    Failed,
    Read,
    OfflineDelivered,
}

#[derive(Serialize, Deserialize, Clone, Debug, Copy, PartialEq, Eq, Hash)]
pub struct MessageId(pub [u8; 16]);
#[allow(clippy::new_without_default)]
impl MessageId {
    pub fn new() -> Self {
        MessageId(uuid::Uuid::new_v7(Timestamp::now(NoContext)).into_bytes())
    }

    pub fn timestamp_secs(&self) -> u64 {
        let b = self.0;
        let ms = ((b[0] as u64) << 40)
            | ((b[1] as u64) << 32)
            | ((b[2] as u64) << 24)
            | ((b[3] as u64) << 16)
            | ((b[4] as u64) << 8)
            | (b[5] as u64);
        ms / 1000
    }
}

#[derive(Serialize, Deserialize)]
pub enum KursalMessage {
    Text(TextMessage),
    Typing,
    ReactionAdd(ReactionAdd),
    ReactionRemove(ReactionRemove),
    MessagePin(MessagePin),
    MessageEdit(MessageEdit),
    MessageDelete(MessageDelete),
    FileOffer(FileOffer),
    FileAccept(FileAccept),
    FileCancel(FileCancel),
    CallSignal(CallSignal),
    DeliveryReceipt(DeliveryReceipt),
    ProfileUpdate(ProfileInfo),
    AddressAnnounce(AddressAnnounce),
    CallRecord(CallRecordMessage),
    ReadReceipt(ReadReceipt),
    ContactTerminate,
}

impl KursalMessage {
    pub fn serialize(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(Into::into)
    }

    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        bincode::deserialize(bytes).map_err(Into::into)
    }

    pub fn message_id(&self) -> Option<MessageId> {
        match self {
            KursalMessage::Text(m) => Some(m.id),
            KursalMessage::FileOffer(m) => Some(m.id),
            KursalMessage::CallSignal(m) => Some(m.call_id),
            KursalMessage::DeliveryReceipt(m) => Some(m.message_id),
            KursalMessage::Typing => None,
            KursalMessage::FileAccept(_) => None,
            KursalMessage::FileCancel(_) => None,
            KursalMessage::ReactionAdd(_) => None,
            KursalMessage::ReactionRemove(_) => None,
            KursalMessage::MessagePin(_) => None,
            KursalMessage::MessageEdit(_) => None,
            KursalMessage::MessageDelete(_) => None,
            KursalMessage::ProfileUpdate(_) => None,
            KursalMessage::AddressAnnounce(_) => None,
            KursalMessage::CallRecord(m) => Some(m.id),
            KursalMessage::ReadReceipt(_) => None,
            KursalMessage::ContactTerminate => None,
        }
    }

    pub fn target_message_id(&self) -> Option<MessageId> {
        match self {
            KursalMessage::MessageEdit(m) => Some(m.target_id),
            KursalMessage::MessageDelete(m) => Some(m.target_id),
            KursalMessage::MessagePin(m) => Some(m.target_id),
            KursalMessage::ReactionAdd(m) => Some(m.target_id),
            KursalMessage::ReactionRemove(m) => Some(m.target_id),
            _ => None,
        }
    }

    pub fn kind_name(&self) -> &'static str {
        match self {
            KursalMessage::Text(_) => "Text",
            KursalMessage::FileOffer(_) => "FileOffer",
            KursalMessage::FileAccept(_) => "FileAccept",
            KursalMessage::FileCancel(_) => "FileCancel",
            KursalMessage::CallSignal(_) => "CallSignal",
            KursalMessage::DeliveryReceipt(_) => "DeliveryReceipt",
            KursalMessage::Typing => "Typing",
            KursalMessage::ReactionAdd(_) => "ReactionAdd",
            KursalMessage::ReactionRemove(_) => "ReactionRemove",
            KursalMessage::MessagePin(_) => "MessagePin",
            KursalMessage::MessageEdit(_) => "MessageEdit",
            KursalMessage::MessageDelete(_) => "MessageDelete",
            KursalMessage::ProfileUpdate(_) => "ProfileUpdate",
            KursalMessage::AddressAnnounce(_) => "AddressAnnounce",
            KursalMessage::CallRecord(_) => "CallRecord",
            KursalMessage::ReadReceipt(_) => "ReadReceipt",
            KursalMessage::ContactTerminate => "ContactTerminate",
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct TextMessage {
    pub id: MessageId,
    pub content: String,
    pub timestamp: u64,
    pub reply_to: Option<MessageId>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub enum CallOutcome {
    Started,
    Completed,
    Missed,
    Declined,
    Busy,
    Canceled,
}

#[derive(Serialize, Deserialize)]
pub struct CallRecordMessage {
    pub id: MessageId,
    pub timestamp: u64,
    pub outcome: CallOutcome,
    pub duration_ms: u64,
}

#[derive(Serialize, Deserialize)]
pub struct ReactionAdd {
    pub target_id: MessageId,
    pub emoji: String,
    pub timestamp: u64,
}

#[derive(Serialize, Deserialize)]
pub struct ReactionRemove {
    pub target_id: MessageId,
    pub emoji: String,
}

#[derive(Serialize, Deserialize)]
pub struct MessagePin {
    pub target_id: MessageId,
    pub pinned: bool,
}

#[derive(Serialize, Deserialize)]
pub struct MessageEdit {
    pub target_id: MessageId,
    pub new_content: String,
    pub edited_at: u64,
}

#[derive(Serialize, Deserialize)]
pub struct MessageDelete {
    pub target_id: MessageId,
}

#[derive(Serialize, Deserialize)]
pub struct FileOffer {
    pub id: MessageId,
    pub filename: String,
    pub size_bytes: u64,
    pub random: [u8; 32],
    pub hash: [u8; 32],
}

#[derive(Serialize, Deserialize)]
pub struct FileAccept {
    pub offer_id: MessageId,
    pub random: [u8; 32],
    pub received_chunks: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct FileCancel {
    pub offer_id: MessageId,
}

#[derive(Serialize, Deserialize)]
pub enum CallSignalKind {
    Offer,
    Answer,
    IceCandidate,
    Hangup,
    VideoStart,
    VideoStop,
    VideoKeyframeRequest,
    VoiceState,
}

#[derive(Serialize, Deserialize)]
pub struct CallSignal {
    pub call_id: MessageId,
    pub kind: CallSignalKind,
    pub payload: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct DeliveryReceipt {
    pub message_id: MessageId,
}

#[derive(Serialize, Deserialize)]
pub struct ReadReceipt {
    pub message_ids: Vec<MessageId>,
}

#[derive(Serialize, Deserialize)]
pub struct AddressAnnounce {
    pub peer_id: String,
    pub addresses: Vec<String>,
}

pub const MAX_PROFILE_AVATAR_LEN: usize = 256 * 1000; // 256 KB
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProfileInfo {
    pub display_name: String,
    pub avatar_bytes: Option<Vec<u8>>,
}
impl ProfileInfo {
    pub fn validate(&self) -> Result<()> {
        if let Some(avatar) = &self.avatar_bytes
            && avatar.len() > MAX_PROFILE_AVATAR_LEN
        {
            return Err(KursalError::Identity(
                "Profile avatar exceeds maximum allowed size.".to_string(),
            ));
        }

        if self.display_name.trim().is_empty() {
            return Err(KursalError::Identity(
                "Profile display name cannot be empty.".to_string(),
            ));
        }

        let name_len = self.display_name.chars().count();
        if !(3..=32).contains(&name_len) {
            return Err(KursalError::Identity(
                "Profile display name must be between 3 and 32 characters.".to_string(),
            ));
        }

        if self.display_name.trim() != self.display_name {
            return Err(KursalError::Identity(
                "Profile display name cannot start or end with whitespace.".to_string(),
            ));
        }

        if !self.display_name.chars().all(is_allowed_name_char) {
            return Err(KursalError::Identity(
                "Profile display name contains unsupported characters.".to_string(),
            ));
        }

        Ok(())
    }
}

fn is_allowed_name_char(c: char) -> bool {
    c.is_alphanumeric() || " ._-'".contains(c)
}
