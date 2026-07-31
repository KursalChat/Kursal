use crate::{
    Result,
    contacts::Contact,
    dto::NetworkStatusDto,
    identity::UserId,
    messaging::{StoredMessage, enums::MessageId},
};
use std::path::PathBuf;
use tokio::sync::oneshot;

pub mod address_announce;
pub mod cmd_wrapper;
pub mod file_transfers;
pub mod handle_core_command;
pub mod handle_incoming;
pub mod message_apply;
pub mod nodes;
pub mod poll_offline;
pub mod send_message;
pub mod state;

pub use address_announce::apply_address_announce;
pub use handle_core_command::handle_core_command;
pub use handle_incoming::handle_incoming;
pub use poll_offline::poll_contact_offline;
pub use send_message::{send_message, send_message_tracked};

#[derive(Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connecting,
    Relay,
    HolePunch,
    Direct,
    Disconnected,
}

pub enum AppEvent {
    MessageReceived {
        contact_id: UserId,
        message: StoredMessage,
        via_offline: bool,
    },
    TypingIndicator {
        contact_id: UserId,
    },
    DeliveryConfirmed {
        contact_id: UserId,
        message_id: MessageId,
    },
    MessagesRead {
        contact_id: UserId,
        message_ids: Vec<MessageId>,
    },
    MessageQueuedOffline {
        contact_id: UserId,
        message_id: MessageId,
    },
    MessagePinned {
        contact_id: UserId,
        message_id: MessageId,
        pinned: bool,
    },
    MessageEdited {
        contact_id: UserId,
        message_id: MessageId,
        new_content: String,
    },
    MessageDeleted {
        contact_id: UserId,
        message_id: MessageId,
    },
    ReactionAdded {
        contact_id: UserId,
        message_id: MessageId,
        emoji: String,
    },
    ReactionRemoved {
        contact_id: UserId,
        message_id: MessageId,
        emoji: String,
    },
    ContactAdded {
        contact: Contact,
    },
    OtpConsumed,
    ContactUpdated {
        contact: Contact,
    },
    ContactRemoved {
        contact_id: UserId,
    },
    ContactTerminated {
        contact_id: UserId,
        terminated: bool,
    },
    ConnectionChange {
        contact_id: UserId,
        status: ConnectionStatus,
    },
    NearbyRequest {
        peer_id: String,
        session_name: String,
        decision_tx: oneshot::Sender<bool>,
    },
    FileOffered {
        contact_id: UserId,
        offer_id: MessageId,
        filename: String,
        size_bytes: u64,
        autodownload: Option<String>,
    },
    FileTransferProgress {
        transfer_id: MessageId,
        bytes_transferred: u64,
        total_bytes: u64,
    },
    FileReceived {
        contact_id: UserId,
        transfer_id: MessageId,
        save_path: String,
    },
    FileTransferFailed {
        contact_id: UserId,
        transfer_id: MessageId,
        reason: String,
    },
    BackendSignal {
        signal: String,
        payload: String,
    },
    NetworkOnline {
        online: bool,
        peer_count: usize,
    },
    OfflineBundlePublished {
        contact_id: UserId,
        message_ids: Vec<MessageId>,
    },
    MessageFailed {
        contact_id: UserId,
        message_ids: Vec<MessageId>,
    },
    OfflineGapSkipped {
        contact_id: UserId,
        counter: u64,
    },
    OfflineQueueDrained {
        contact_id: UserId,
    },
    OfflineSync {
        active: bool,
    },
    CallIncoming {
        call_id: MessageId,
        contact_id: UserId,
        sample_rate: u32,
    },
    CallState {
        call_id: MessageId,
        state: String,
    },
    CallLevel {
        mic: f32,
        remote: f32,
    },
    CallEnded {
        call_id: MessageId,
        reason: String,
        duration_ms: u64,
    },
    VideoCongestion,
    VideoState {
        call_id: MessageId,
        active: bool,
        codec: Option<String>,
        width: u16,
        height: u16,
        reason: Option<String>,
    },
    VideoKeyframeRequested {
        call_id: MessageId,
    },
    CallPeerVoiceState {
        call_id: MessageId,
        muted: bool,
        deafened: bool,
    },
}

pub enum CoreCommand {
    PublishOtp {
        otp: String,
        reply: oneshot::Sender<Result<()>>,
    },
    FetchOtp {
        otp: String,
        reply: oneshot::Sender<Result<Contact>>,
    },
    ExportLtc {
        reply: oneshot::Sender<Result<Vec<u8>>>,
    },
    ImportLtc {
        bytes: Vec<u8>,
        reply: oneshot::Sender<Result<Contact>>,
    },
    ConnectNearby {
        peer_id: String,
        session_name: String,
        method: String,
        reply: oneshot::Sender<Result<()>>,
    },
    SendText {
        contact_id: String,
        text: String,
        reply_to: Option<MessageId>,
        reply: oneshot::Sender<Result<MessageId>>,
    },
    SendTypingIndicator {
        contact_id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    SendReadReceipts {
        contact_id: String,
        message_ids: Vec<String>,
        reply: oneshot::Sender<Result<()>>,
    },
    RotatePeerId {
        reply: oneshot::Sender<Result<()>>,
    },
    AnnounceAddresses,
    RemoveContact {
        contact_id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    DeleteMessage {
        contact_id: String,
        message_id: String,
        reply: oneshot::Sender<Result<bool>>,
    },
    PinMessage {
        contact_id: String,
        message_id: String,
        pinned: bool,
        reply: oneshot::Sender<Result<bool>>,
    },
    EditMessage {
        contact_id: String,
        message_id: String,
        new_content: String,
        reply: oneshot::Sender<Result<bool>>,
    },
    ReactionAdd {
        contact_id: String,
        message_id: String,
        emoji: String,
        reply: oneshot::Sender<Result<bool>>,
    },
    ReactionRemove {
        contact_id: String,
        message_id: String,
        emoji: String,
        reply: oneshot::Sender<Result<bool>>,
    },
    DeleteLocalMessage {
        contact_id: String,
        message_id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    RetryMessage {
        contact_id: String,
        message_id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    ShareProfile {
        contact_id: String,
        display_name: String,
        avatar_bytes: Option<Vec<u8>>,
        reply: oneshot::Sender<Result<()>>,
    },
    BroadcastProfile {
        display_name: String,
        avatar_bytes: Option<Vec<u8>>,
        reply: oneshot::Sender<Result<()>>,
    },
    SendFileOffer {
        contact_id: String,
        file_path: String,
        app_data_dir: PathBuf,
        reply: oneshot::Sender<Result<(MessageId, u64, String)>>,
    },
    AcceptFileOffer {
        contact_id: String,
        offer_id: String,
        save_path: String,
        reply: oneshot::Sender<Result<()>>,
    },
    CancelFileTransfer {
        contact_id: String,
        offer_id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    FlushOffline {
        contact_id: String,
        reply: oneshot::Sender<Result<()>>,
    },
    AddCustomNode {
        addr: String,
        reply: oneshot::Sender<Result<()>>,
    },
    RemoveCustomNode {
        addr: String,
        reply: oneshot::Sender<Result<()>>,
    },
    DialAddress {
        addr: String,
        reply: oneshot::Sender<Result<()>>,
    },
    NetworkStatus {
        reply: oneshot::Sender<Result<NetworkStatusDto>>,
    },
    #[cfg(feature = "calls")]
    StartCall {
        contact_id: String,
        reply: oneshot::Sender<Result<MessageId>>,
    },
    #[cfg(feature = "calls")]
    AcceptCall {
        reply: oneshot::Sender<Result<()>>,
    },
    #[cfg(feature = "calls")]
    DeclineCall {
        reply: oneshot::Sender<Result<()>>,
    },
    #[cfg(feature = "calls")]
    HangupCall {
        reply: oneshot::Sender<Result<()>>,
    },
    #[cfg(feature = "calls")]
    StartVideo {
        codec: String,
        width: u16,
        height: u16,
        reply: oneshot::Sender<Result<()>>,
    },
    #[cfg(feature = "calls")]
    StopVideo {
        reason: String,
        reply: oneshot::Sender<Result<()>>,
    },
    #[cfg(feature = "calls")]
    RequestVideoKeyframe {
        reply: oneshot::Sender<Result<()>>,
    },
    #[cfg(feature = "calls")]
    SetMute {
        muted: bool,
        reply: oneshot::Sender<Result<()>>,
    },
    #[cfg(feature = "calls")]
    SetDeafen {
        deafened: bool,
        reply: oneshot::Sender<Result<()>>,
    },
    #[cfg(feature = "calls")]
    SetAudioDevice {
        kind: String,
        name: Option<String>,
        reply: oneshot::Sender<Result<()>>,
    },
    #[cfg(feature = "calls")]
    ListAudioDevices {
        reply: oneshot::Sender<Result<crate::call::audio::AudioDevices>>,
    },
}
