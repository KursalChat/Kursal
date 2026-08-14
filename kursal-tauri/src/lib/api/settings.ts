import { invoke } from '@tauri-apps/api/core';

export type PeerRotationInterval = '6h' | '12h' | '30h' | '7d' | 'manual';

export type AppLockMethod = 'none' | 'password' | 'biometric';
export interface AppLockConfig {
  enabled: boolean;
  method: AppLockMethod;
}

export interface RelayConfig {
  enabled: boolean;
  maxConnections: number;
  maxConnectionsPerIp: number;
}

export type AutoAcceptMode = 'nobody' | 'verified' | 'all';
export interface AutoAcceptConfig {
  mode: AutoAcceptMode;
  sizeCapBytes: number;
}

export type AutoDownloadScope = 'per_contact' | 'all_contacts';
export interface AutoDownloadConfig {
  scope: AutoDownloadScope;
  limitBytes: number;
}

export interface SharedFileEntry {
  id: string;
  filepath: string;
  sizeBytes: number;
  recipientId: string;
  sharedAt: number;
  lastAccessedAt: number | null;
}

export interface ContactUsage {
  contactId: string;
  dbBytes: number;
  filesBytes: number;
}
export interface StorageUsage {
  logsBytes: number;
  dbBytes: number;
  filesBytes: number;
  avatarsBytes: number;
  perContact: ContactUsage[];
}

export interface LocalApiConfig {
  enabled: boolean;
  hostOnNetwork: boolean;
  port: number;
}

export const getPeerRotationInterval = (): Promise<PeerRotationInterval> =>
  invoke('get_peer_rotation_interval');
export const setPeerRotationInterval = (interval: PeerRotationInterval): Promise<void> =>
  invoke('set_peer_rotation_interval', { interval });

export const getAppLockConfig = (): Promise<AppLockConfig> => invoke('get_app_lock_config');
export const setAppLock = (
  enabled: boolean,
  method: AppLockMethod,
  password: string | null
): Promise<void> => invoke('set_app_lock', { enabled, method, password });
export const verifyAppLock = (password: string): Promise<boolean> =>
  invoke('verify_app_lock', { password });

export const getTypingIndicatorsEnabled = (): Promise<boolean> =>
  invoke('get_typing_indicators_enabled');
export const setTypingIndicatorsEnabled = (enabled: boolean): Promise<void> =>
  invoke('set_typing_indicators_enabled', { value: enabled });

export const getReadReceiptsEnabled = (): Promise<boolean> => invoke('get_read_receipts_enabled');
export const setReadReceiptsEnabled = (enabled: boolean): Promise<void> =>
  invoke('set_read_receipts_enabled', { value: enabled });

export const listBlockedContacts = (): Promise<import('$lib/types').ContactResponse[]> =>
  invoke('list_blocked_contacts');

export const clearMessageHistory = (contactId: string | null): Promise<void> =>
  invoke('clear_message_history', { contactId });
export const deleteAllLocalData = (): Promise<void> => invoke('delete_all_local_data');

export const getRelayConfig = (): Promise<RelayConfig> => invoke('get_relay_config');
export const setRelayConfig = (config: RelayConfig): Promise<void> =>
  invoke('set_relay_config', { config });

// Nodes (bootstrap/relay addresses)
export interface NodesResponse {
  defaults: string[];
  custom: string[];
}
export const getNodes = (): Promise<NodesResponse> => invoke('get_nodes');
export const addCustomNode = (addr: string): Promise<void> => invoke('add_custom_node', { addr });
export const removeCustomNode = (addr: string): Promise<void> =>
  invoke('remove_custom_node', { addr });
export const dialAddress = (addr: string): Promise<void> => invoke('dial_address', { addr });

export interface NetworkStatus {
  peerCount: number;
  connectedPeers: string[];
  listenAddresses: string[];
}
export const getNetworkStatus = (): Promise<NetworkStatus> => invoke('get_network_status');

export interface NodeStats {
  cpuPercent: number;
  memBytes: number;
  uptimeSecs: number;
  bytesIn: number;
  bytesOut: number;
  rateIn: number;
  rateOut: number;
}
export const getNodeStats = (): Promise<NodeStats> => invoke('get_node_stats');

/** Core emits `node_stats` on its own timer until stopNodeStats(). */
export const startNodeStats = (): Promise<void> => invoke('start_node_stats');
export const stopNodeStats = (): Promise<void> => invoke('stop_node_stats');

export const getListeningPort = (): Promise<number | null> => invoke('get_listening_port');
export const setListeningPort = (port: number | null): Promise<void> =>
  invoke('set_listening_port', { port });

export const getNearbyShareEnabled = (): Promise<boolean> => invoke('get_nearby_share_enabled');
export const setNearbyShareEnabled = (enabled: boolean): Promise<void> =>
  invoke('set_nearby_share_enabled', { value: enabled });

export const listSharedFiles = (): Promise<SharedFileEntry[]> => invoke('list_shared_files');
export const revokeSharedFile = (id: string): Promise<void> => invoke('revoke_shared_file', { id });
export const revokeSharedFilesBulk = (ids: string[]): Promise<void> =>
  invoke('revoke_shared_files_bulk', { ids });

export const getAutoAcceptConfig = (): Promise<AutoAcceptConfig> =>
  invoke('get_auto_accept_config');
export const setAutoAcceptConfig = (config: AutoAcceptConfig): Promise<void> =>
  invoke('set_auto_accept_config', { config });

export const getAutoDownloadConfig = (): Promise<AutoDownloadConfig> =>
  invoke('get_auto_download_config');
export const setAutoDownloadConfig = (config: AutoDownloadConfig): Promise<void> =>
  invoke('set_auto_download_config', { config });

export const getStorageUsage = (): Promise<StorageUsage> => invoke('get_storage_usage');

export const checkForUpdates = (): Promise<void> => invoke('check_for_updates');
export const getUpdaterEnabled = (): Promise<boolean> => invoke('get_updater_enabled');
export const setUpdaterEnabled = (value: boolean): Promise<void> =>
  invoke('set_updater_enabled', { value });

export type UpdateChannel = 'stable' | 'beta';
export const getUpdateChannel = (): Promise<UpdateChannel> => invoke('get_update_channel');
export const setUpdateChannel = (channel: UpdateChannel): Promise<void> =>
  invoke('set_update_channel', { channel });

export const getBackgroundMode = (): Promise<boolean> => invoke('get_background_mode');
export const setBackgroundMode = (value: boolean): Promise<void> =>
  invoke('set_background_mode', { value });

// Close guards: the backend owns the close decision and asks the UI to confirm
// whenever a call or a file transfer is still running.
export const setBusyState = (callActive: boolean, transferActive: boolean): Promise<void> =>
  invoke('set_busy_state', { callActive, transferActive });
export const closeToBackground = (untilIdle: boolean): Promise<void> =>
  invoke('close_to_background', { untilIdle });
export const closeForceQuit = (): Promise<void> => invoke('close_force_quit');
// Mirrors the "user has never seen the close explainer" flag down to the backend,
// which decides on the close path and cannot read localStorage.
export const setCloseExplainerPending = (value: boolean): Promise<void> =>
  invoke('set_close_explainer_pending', { value });

// Notification preview + DND mirrored to core (for window-closed notifications)
export const setNotificationPreviewCore = (value: string): Promise<void> =>
  invoke('set_notification_preview', { value });
export const setNotificationDndCore = (value: string): Promise<void> =>
  invoke('set_notification_dnd', { value });

// Encrypted UI state (drafts, pinned convos, ...) stored in the settings table
export const getUiState = (key: string): Promise<string | null> => invoke('get_ui_state', { key });
export const setUiState = (key: string, value: string): Promise<void> =>
  invoke('set_ui_state', { key, value });

export const getLocalApiConfig = (): Promise<LocalApiConfig> => invoke('get_local_api_config');
export const setLocalApiConfig = (config: LocalApiConfig): Promise<void> =>
  invoke('set_local_api_config', { config });
export const generateLocalApiToken = (): Promise<string> => invoke('generate_local_api_token');
