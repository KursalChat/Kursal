use crate::{
    Result,
    contacts::Contact,
    dto::{LtcStatusDto, NetworkStatusDto},
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
pub mod profile_share;
pub mod send_message;
pub mod state;

pub use address_announce::apply_address_announce;
pub use handle_core_command::handle_core_command;
pub use handle_incoming::handle_incoming;
pub use poll_offline::{PollTrigger, poll_contact_offline};
pub use profile_share::{resend_stale_profile, share_profile_with};
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
        via_nearby: bool,
    },
    LtcUpdated {
        status: Option<LtcStatusDto>,
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
        decision_tx: Reply<bool>,
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
    ReachabilityChanged {
        reachability: String,
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
        finalized_deletes: Vec<MessageId>,
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
    VideoLocalState {
        active: bool,
        codec: Option<String>,
        width: u16,
        height: u16,
        camera_id: Option<String>,
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

type Reply<T> = oneshot::Sender<T>;

pub enum CoreCommand {
    PublishOtp {
        otp: String,
        reply: Reply<Result<()>>,
    },
    FetchOtp {
        otp: String,
        reply: Reply<Result<Contact>>,
    },
    GetLtcStatus {
        reply: Reply<Result<Option<LtcStatusDto>>>,
    },
    CreateLtc {
        max_uses: Option<u32>,
        ttl_secs: Option<u64>,
        reply: Reply<Result<LtcStatusDto>>,
    },
    UpdateLtcLimits {
        max_uses: Option<u32>,
        ttl_secs: Option<u64>,
        reply: Reply<Result<LtcStatusDto>>,
    },
    ExportLtc {
        reply: Reply<Result<Vec<u8>>>,
    },
    SetLtcFollowRotations {
        enabled: bool,
        reply: Reply<Result<LtcStatusDto>>,
    },
    RepublishLtcPointer {
        reply: Reply<Result<LtcStatusDto>>,
    },
    RevokeLtc {
        reply: Reply<Result<()>>,
    },
    ImportLtc {
        bytes: Vec<u8>,
        reply: Reply<Result<Contact>>,
    },
    ConnectNearby {
        peer_id: String,
        session_name: String,
        method: String,
        reply: Reply<Result<()>>,
    },
    SendText {
        contact_id: String,
        text: String,
        reply_to: Option<MessageId>,
        reply: Reply<Result<MessageId>>,
    },
    SendTypingIndicator {
        contact_id: String,
        reply: Reply<Result<()>>,
    },
    SendReadReceipts {
        contact_id: String,
        message_ids: Vec<String>,
        reply: Reply<Result<()>>,
    },
    RotatePeerId {
        reply: Reply<Result<()>>,
    },
    AnnounceAddresses,
    RemoveContact {
        contact_id: String,
        reply: Reply<Result<()>>,
    },
    DeleteMessage {
        contact_id: String,
        message_id: String,
        reply: Reply<Result<bool>>,
    },
    PinMessage {
        contact_id: String,
        message_id: String,
        pinned: bool,
        reply: Reply<Result<bool>>,
    },
    EditMessage {
        contact_id: String,
        message_id: String,
        new_content: String,
        reply: Reply<Result<bool>>,
    },
    ReactionAdd {
        contact_id: String,
        message_id: String,
        emoji: String,
        reply: Reply<Result<bool>>,
    },
    ReactionRemove {
        contact_id: String,
        message_id: String,
        emoji: String,
        reply: Reply<Result<bool>>,
    },
    DeleteLocalMessage {
        contact_id: String,
        message_id: String,
        reply: Reply<Result<()>>,
    },
    RetryMessage {
        contact_id: String,
        message_id: String,
        reply: Reply<Result<()>>,
    },
    ShareProfile {
        contact_id: String,
        display_name: String,
        avatar_bytes: Option<Vec<u8>>,
        reply: Reply<Result<()>>,
    },
    BroadcastProfile {
        display_name: String,
        avatar_bytes: Option<Vec<u8>>,
        reply: Reply<Result<()>>,
    },
    SendFileOffer {
        contact_id: String,
        file_path: String,
        app_data_dir: PathBuf,
        reply: Reply<Result<(MessageId, u64, String)>>,
    },
    AcceptFileOffer {
        contact_id: String,
        offer_id: String,
        save_path: String,
        reply: Reply<Result<()>>,
    },
    CancelFileTransfer {
        contact_id: String,
        offer_id: String,
        reply: Reply<Result<()>>,
    },
    FlushOffline {
        contact_id: String,
        reply: Reply<Result<()>>,
    },
    AddCustomNode {
        addr: String,
        reply: Reply<Result<()>>,
    },
    RemoveCustomNode {
        addr: String,
        reply: Reply<Result<()>>,
    },
    DialAddress {
        addr: String,
        reply: Reply<Result<()>>,
    },
    NetworkStatus {
        reply: Reply<Result<NetworkStatusDto>>,
    },
    #[cfg(feature = "calls")]
    StartCall {
        contact_id: String,
        reply: Reply<Result<MessageId>>,
    },
    #[cfg(feature = "calls")]
    AcceptCall {
        reply: Reply<Result<()>>,
    },
    #[cfg(feature = "calls")]
    DeclineCall {
        reply: Reply<Result<()>>,
    },
    #[cfg(feature = "calls")]
    HangupCall {
        reply: Reply<Result<()>>,
    },
    #[cfg(feature = "calls")]
    StartVideo {
        reply: Reply<Result<()>>,
    },
    #[cfg(feature = "calls")]
    ListCameras {
        reply: Reply<Result<Vec<crate::dto::CameraInfo>>>,
    },
    #[cfg(feature = "calls")]
    SetCamera {
        camera_id: Option<String>,
        reply: Reply<Result<()>>,
    },
    #[cfg(feature = "calls")]
    RefreshCameraRotation {
        angle: u16,
        reply: Reply<Result<()>>,
    },
    #[cfg(feature = "calls")]
    RequestLocalKeyframe {
        reply: Reply<Result<()>>,
    },
    #[cfg(feature = "calls")]
    StopVideo {
        reason: String,
        reply: Reply<Result<()>>,
    },
    #[cfg(feature = "calls")]
    RequestVideoKeyframe {
        reply: Reply<Result<()>>,
    },
    #[cfg(feature = "calls")]
    SetMute {
        muted: bool,
        reply: Reply<Result<()>>,
    },
    #[cfg(feature = "calls")]
    SetDeafen {
        deafened: bool,
        reply: Reply<Result<()>>,
    },
    #[cfg(feature = "calls")]
    SetAudioDevice {
        kind: String,
        name: Option<String>,
        reply: Reply<Result<()>>,
    },
    #[cfg(feature = "calls")]
    ListAudioDevices {
        reply: Reply<Result<crate::call::audio::AudioDevices>>,
    },
}
