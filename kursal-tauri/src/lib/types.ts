export interface ContactResponse {
  userId: string; // hex of UserId bytes
  displayName: string;
  peerId: string;
  knownAddresses: string[];
  verified: boolean;
  profileShared: boolean;
  blocked: boolean;
  createdAt: number;
  avatarBase64?: string | null; // base64 encoded webp string
  avatarBytes?: number[] | null; // Raw byte array from Rust
  profileName?: string; // peer-chosen name; displayName may be a local alias
}

export interface ContactMeta {
  contactId: string;
  muted: boolean;
  lastSeenAt: number | null; // ms epoch, backend-stamped on connection events
  alias: string | null; // local nickname, overrides displayName in the UI
  // Peer removed us as a contact. History stays, but they can no longer be
  // reached, so the composer is closed until the contact is re-established.
  terminated: boolean;
}

export interface ContactTerminatedPayload {
  contactId: string;
  terminated: boolean;
}

export interface MessageResponse {
  id: string; // hex of MessageId bytes
  contactId: string; // hex of UserId bytes
  direction: 'sent' | 'received';
  content: string;
  // Sent-message lifecycle:
  //   sending           - DR ciphertext is in flight (direct path attempt)
  //   queued            - backend confirmed offline path; awaiting peer to fetch
  //   delivered         - receipt back, peer was reachable directly
  //   offline_delivered - receipt back, peer fetched via DHT-backed offline channel
  //   failed            - gave up
  status:
    'sending' | 'delivered' | 'failed' | 'queued' | 'queued_in_dht' | 'offline_delivered' | 'read';
  // Received messages only: true when this message was fetched from the
  // DHT-backed offline channel rather than a live direct connection. Set by
  // the backend on the offline receive path; session-only (not persisted, so
  // it won't reappear on history reload - same degradation as offline_delivered).
  viaOffline?: boolean;
  timestamp: number; // sent time (derived from MessageId), ms after hydration
  receivedTimestamp: number; // local receive time, ms after hydration
  replyTo: string | null; // hex of MessageId bytes
  reactions?: { emoji: string; userId: string }[];
  edited?: boolean;
  pinned?: boolean;
  fileDetails?: {
    filename: string;
    sizeBytes: number;
    autodownloadPath?: string | null;
  } | null;
  callDetails?: {
    outcome: string;
    durationMs: number;
  } | null;
}

export interface OtpResponse {
  otp: string;
}

export type NearbyOrigin = 'Bluetooth' | 'mDNS';

export interface NearbyPeerResponse {
  peerId: string;
  sessionName: string;
  origin: NearbyOrigin;
}

// Tauri event payloads - mirror what the Rust AppEvent forwarder emits
export type MessageReceivedPayload = MessageResponse;

export interface ConnectionChangedPayload {
  contactId: string;
  status: 'connecting' | 'relay' | 'holepunch' | 'direct' | 'disconnected';
}

export interface BackendSignalPayload {
  signal: string;
  payload: string;
}

export interface NearbyRequestPayload {
  peerId: string;
  sessionName: string;
}

export interface PeerIdHolderPayload {
  peerId: string;
}

export interface NetworkOnlinePayload {
  online: boolean;
  peerCount: number;
}

export interface OfflineBundlePublishedPayload {
  contactId: string;
  messageIds: string[];
}

export interface OfflineGapSkippedPayload {
  contactId: string;
  counter: number;
}

export interface OfflineQueueDrainedPayload {
  contactId: string;
}

export interface OfflineSyncPayload {
  active: boolean;
}

export interface CloseRequestedPayload {
  callActive: boolean;
  transferActive: boolean;
  // Set when the backend blocked the close only to let the UI explain that
  // closing leaves Kursal running in the tray. Fires once, ever.
  firstClose?: boolean;
}

export interface MessageEditedPayload {
  contactId: string;
  messageId: string;
  newContent: string;
}

export interface MessageDeletedPayload {
  contactId: string;
  messageId: string;
}

export interface MessageQueuedOfflinePayload {
  contactId: string;
  messageId: string;
}

export interface MessagesReadPayload {
  contactId: string;
  messageIds: string[];
}

export interface ReactionChangedPayload {
  contactId: string;
  messageId: string;
  emoji: string;
  userId?: string;
}

export interface FileOfferedPayload {
  offerId: string;
  contactId: string;
  filename: string;
  sizeBytes: number;
  autodownload: string | null; // if specified, path to the auto downloaded
}

export interface FileTransferProgressPayload {
  transferId: string;
  bytesTransferred: number;
  totalBytes: number;
}

export interface FileReceivedPayload {
  contactId: string;
  transferId: string;
  savePath: string;
}

export interface FileTransferFailedPayload {
  contactId: string;
  transferId: string;
  reason: string;
}

export interface TypingIndicatorPayload {
  contactId: string;
  replyTo: string | null;
}

export type CallStatus =
  'idle' | 'ringing_out' | 'ringing_in' | 'connecting' | 'connected' | 'ended';

export interface CallIncomingPayload {
  callId: string;
  contactId: string;
  sampleRate: number;
}

export interface CallStatePayload {
  callId: string;
  state: CallStatus;
}

export interface CallLevelPayload {
  mic: number;
  remote: number;
}

export interface CallEndedPayload {
  callId: string;
  reason: string;
  durationMs: number;
}

export interface VideoStatePayload {
  callId: string;
  active: boolean;
  codec: string | null;
  width: number;
  height: number;
  reason: string | null;
}

export interface CallPeerVoiceStatePayload {
  callId: string;
  muted: boolean;
  deafened: boolean;
}

export interface AudioDevices {
  inputs: string[];
  outputs: string[];
  selectedInput: string | null;
  selectedOutput: string | null;
}
