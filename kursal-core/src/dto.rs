use crate::{
    contacts::Contact,
    first_contact::nearby::{NearbyBeacon, NearbyOrigin},
    messaging::{
        StoredMessage,
        enums::{CallOutcome, Direction, KursalMessage, MessageStatus},
    },
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContactResponse {
    pub user_id: String,
    pub display_name: String,
    pub avatar_bytes: Option<Vec<u8>>,
    pub peer_id: String,
    pub known_addresses: Vec<String>,
    pub verified: bool,
    pub profile_shared: bool,
    pub blocked: bool,
    pub created_at: u64,
}

impl From<Contact> for ContactResponse {
    fn from(value: Contact) -> Self {
        Self {
            user_id: hex::encode(value.user_id.0),
            display_name: value.display_name,
            avatar_bytes: value.avatar_bytes,
            peer_id: value.peer_id,
            known_addresses: value.known_addresses,
            verified: value.verified,
            profile_shared: value.profile_shared,
            blocked: value.blocked,
            created_at: value.created_at,
        }
    }
}
#[derive(Serialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReactionResponse {
    pub emoji: String,
    pub user_id: String,
}

#[derive(Serialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileDetailsDto {
    pub filename: String,
    pub size_bytes: u64,
}

#[derive(Serialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CallDetailsDto {
    pub outcome: String,
    pub duration_ms: u64,
}

#[derive(Serialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PinDetailsDto {
    pub target_id: String,
    pub pinned: bool,
}

#[derive(Serialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MessageResponse {
    pub id: String,
    pub contact_id: String,
    pub direction: String,
    pub content: String,
    pub status: String,
    pub timestamp: u64,
    pub received_timestamp: u64,
    pub reply_to: Option<String>,
    pub edited: bool,
    pub pinned: bool,
    pub reactions: Vec<ReactionResponse>,
    pub file_details: Option<FileDetailsDto>,
    pub call_details: Option<CallDetailsDto>,
    pub pin_details: Option<PinDetailsDto>,
    pub via_offline: bool,
}

impl From<StoredMessage> for MessageResponse {
    fn from(value: StoredMessage) -> Self {
        let reply_to = match &value.payload {
            KursalMessage::Text(t) => t.reply_to.map(|id| hex::encode(id.0)),
            _ => None,
        };

        let db_reactions: Vec<ReactionResponse> = value
            .reactions
            .into_iter()
            .map(|r| ReactionResponse {
                emoji: r.emoji,
                user_id: hex::encode(r.user_id.0),
            })
            .collect();

        let file_details = match &value.payload {
            KursalMessage::FileOffer(f) => Some(FileDetailsDto {
                filename: f.filename.clone(),
                size_bytes: f.size_bytes,
            }),
            _ => None,
        };

        let call_details = match &value.payload {
            KursalMessage::CallRecord(c) => Some(CallDetailsDto {
                outcome: match c.outcome {
                    CallOutcome::Started => "started",
                    CallOutcome::Completed => "completed",
                    CallOutcome::Missed => "missed",
                    CallOutcome::Declined => "declined",
                    CallOutcome::Busy => "busy",
                    CallOutcome::Canceled => "canceled",
                }
                .to_string(),
                duration_ms: c.duration_ms,
            }),
            _ => None,
        };

        let pin_details = match &value.payload {
            KursalMessage::MessagePin(p) => Some(PinDetailsDto {
                target_id: hex::encode(p.target_id.0),
                pinned: p.pinned,
            }),
            _ => None,
        };

        Self {
            id: hex::encode(value.id.0),
            contact_id: hex::encode(value.contact_id.0),
            direction: match value.direction {
                Direction::Received => "received".to_string(),
                Direction::Sent => "sent".to_string(),
            },
            content: match &value.payload {
                KursalMessage::Text(t) => t.content.clone(),
                KursalMessage::ReactionAdd(r) => format!("Reacted {} to a message", r.emoji),
                KursalMessage::ReactionRemove(r) => format!("Removed reaction {}", r.emoji),
                KursalMessage::MessageEdit(e) => format!("Edited {}", e.new_content),
                KursalMessage::MessageDelete(_) => String::new(),
                KursalMessage::FileOffer(f) => format!("File: {}", f.filename),
                KursalMessage::FileAccept(_) => "[file offer reply]".to_string(),
                KursalMessage::FileCancel(_) => "[file cancelled]".to_string(),
                KursalMessage::CallSignal(_) => "[call]".to_string(),
                KursalMessage::DeliveryReceipt(_) => "[receipt]".to_string(),
                KursalMessage::ProfileUpdate(_) => "[profile updated]".to_string(),
                KursalMessage::Typing => "[user typing]".to_string(),
                KursalMessage::MessagePin(_) => String::new(),
                KursalMessage::AddressAnnounce(_) => "[address update]".to_string(),
                KursalMessage::CallRecord(_) => String::new(),
                KursalMessage::ReadReceipt(_) => "[receipt]".to_string(),
                KursalMessage::ContactTerminate => "[contact removed]".to_string(),
            },
            status: match value.status {
                MessageStatus::Delivered => "delivered".to_string(),
                MessageStatus::Failed => "failed".to_string(),
                MessageStatus::Sending => "sending".to_string(),
                MessageStatus::Read => "read".to_string(),
                MessageStatus::OfflineDelivered => match value.direction {
                    Direction::Sent => "offline_delivered".to_string(),
                    Direction::Received => "delivered".to_string(),
                },
            },
            timestamp: value.id.timestamp_secs(),
            received_timestamp: value.timestamp,
            reply_to,
            edited: value.edited,
            pinned: value.pinned,
            reactions: db_reactions,
            file_details,
            call_details,
            pin_details,
            via_offline: matches!(value.status, MessageStatus::OfflineDelivered)
                && matches!(value.direction, Direction::Received),
        }
    }
}

pub fn apply_offline_overlay(
    rows: &mut [MessageResponse],
    offline: &crate::messaging::offline::OfflineState,
) {
    let queued: std::collections::HashSet<String> = offline
        .send_queue
        .iter()
        .filter_map(|q| q.message_id)
        .map(|id| hex::encode(id.0))
        .collect();
    let bundled: std::collections::HashSet<String> = offline
        .pending_bundles
        .iter()
        .flat_map(|b| b.message_ids.iter().copied())
        .map(|id| hex::encode(id.0))
        .collect();

    for row in rows.iter_mut() {
        if row.direction != "sent" || row.status != "sending" {
            continue;
        }
        if queued.contains(&row.id) {
            row.status = "queued".to_string();
        } else if bundled.contains(&row.id) {
            row.status = "queued_in_dht".to_string();
        }
    }
}

#[derive(Serialize, Deserialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OtpResponse {
    pub otp: String,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum LtcPointerState {
    Disabled,
    Pending,
    Published,
    Failed,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LtcStatusDto {
    pub payload_id: String,
    pub created_at: u64,
    pub expires_at: Option<u64>,
    pub max_uses: Option<u32>,
    pub uses: u32,
    pub size_bytes: u32,
    pub follow_rotations: bool,
    pub pointer_state: LtcPointerState,
}

#[derive(Serialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodesResponse {
    pub defaults: Vec<String>,
    pub custom: Vec<String>,
}

#[derive(Serialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStatusDto {
    pub peer_count: usize,
    pub connected_peers: Vec<String>,
    pub listen_addresses: Vec<String>,
}

#[derive(Serialize, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NearbyPeerResponse {
    pub peer_id: String,
    pub session_name: String,
    pub origin: NearbyOrigin,
}

impl From<(NearbyBeacon, NearbyOrigin)> for NearbyPeerResponse {
    fn from((value, origin): (NearbyBeacon, NearbyOrigin)) -> Self {
        Self {
            peer_id: value.peer_id,
            session_name: value.session_name,
            origin,
        }
    }
}
