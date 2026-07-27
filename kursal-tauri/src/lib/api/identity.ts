import { invoke } from '@tauri-apps/api/core';

export const rotatePeerId = (): Promise<void> => invoke('rotate_peer_id');

export const getLocalPeerId = (): Promise<string> => invoke('get_local_peer_id');

export const getLocalUserId = (): Promise<string> => invoke('get_local_user_id_hex');

export const getLocalUserProfile = (): Promise<[string, number[] | null]> =>
  invoke('get_local_user_profile');

export const broadcastProfile = (
  displayName: string,
  avatarBytes: number[] | null
): Promise<void> => invoke('broadcast_profile', { displayName, avatarBytes });

export const shareProfile = (
  displayName: string,
  avatarBytes: number[] | null,
  contactId: string
): Promise<void> => invoke('share_profile', { displayName, avatarBytes, contactId });

export const frontendReady = (): Promise<void> => invoke('frontend_ready');
