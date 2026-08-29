export interface ContactResponse {
  userId: string; // hex of UserId bytes
  displayName: string;
  peerId: string;
  knownAddresses: string[];
  verified: boolean;
  profileShared: boolean;
  blocked: boolean;
  createdAt: number;
  avatarPath?: string | null; // absolute path to <userId>.webp, with a `?v=<mtime>` cache buster
  profileName?: string; // peer-chosen name; displayName may be a local alias
}

export interface ContactMeta {
  contactId: string;
  muted: boolean;
  lastSeenAt: number | null; // ms epoch, backend-stamped on connection events
  lastMessageAt: number | null; // unix seconds of the newest stored message
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
  // Sent-message lifecycle: sending (DR ciphertext in flight), queued (offline path
  // confirmed, awaiting fetch), delivered (peer reachable directly), offline_delivered
  // (peer fetched via DHT), failed (gave up).
  status:
    | 'sending'
    | 'delivered'
    | 'failed'
    | 'queued'
    | 'queued_in_dht'
    | 'offline_delivered'
    | 'read';
  // Received messages only: true when fetched via the DHT-backed offline channel
  // rather than a live connection. Session-only, not persisted, so it won't
  // reappear on history reload; same degradation as offline_delivered.
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
  pinDetails?: {
    targetId: string;
    pinned: boolean;
  } | null;
}

export interface UnreadEntry {
  contactId: string;
  count: number;
  // `count` stopped at the scan cap; the real total is higher.
  capped: boolean;
  firstUnread: string | null;
  markedUnread: boolean;
}

export interface PendingSyncSnapshot {
  // `contactId:messageId` pairs.
  sync: string[];
  deleted: string[];
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

// Tauri event payloads: mirrors what the Rust AppEvent forwarder emits
export type MessageReceivedPayload = MessageResponse;

export type ConfirmTone = 'default' | 'warning' | 'danger';

export interface BackendDialogPayload {
  id: number;
  kind: string;
  message?: string;
  params: Record<string, string | number | boolean | null>;
  tone: ConfirmTone;
  dismissible: boolean;
}

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
  finalizedDeletes: string[];
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

export interface UpdateDownloadProgressPayload {
  downloaded: number;
  contentLength: number | null;
}

export type CallStatus =
  | 'idle'
  | 'ringing_out'
  | 'ringing_in'
  | 'connecting'
  | 'connected'
  | 'ended';

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

export interface CameraInfo {
  id: string;
  label: string;
  facing: string | null;
}

export interface VideoLocalStatePayload {
  active: boolean;
  codec: string | null;
  width: number;
  height: number;
  cameraId: string | null;
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

export interface ShareFile {
  path: string;
  filename: string;
  sizeBytes: number;
}

export interface SharePayload {
  id: string;
  files: ShareFile[];
  text: string | null;
}

export type LtcPointerState = 'disabled' | 'pending' | 'published' | 'failed';

export interface LtcStatus {
  payloadId: string;
  createdAt: number; // unix seconds, not ms
  expiresAt: number | null; // unix seconds; null = never
  maxUses: number | null; // null = unlimited
  uses: number;
  sizeBytes: number;
  followRotations: boolean;
  pointerState: LtcPointerState;
}
