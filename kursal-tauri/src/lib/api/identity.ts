import { invoke } from '@tauri-apps/api/core';

export const rotatePeerId = (): Promise<void> => invoke('rotate_peer_id');

export const getLocalPeerId = (): Promise<string> => invoke('get_local_peer_id');

export const getLocalUserId = (): Promise<string> => invoke('get_local_user_id_hex');

export const getLocalUserProfile = (): Promise<[string, string | null]> =>
  invoke('get_local_user_profile');

export const setLocalUserAvatar = (avatarBytes: number[] | null): Promise<string | null> =>
  invoke('set_local_user_avatar', { avatarBytes });

export const broadcastProfile = (displayName: string): Promise<void> =>
  invoke('broadcast_profile', { displayName });

export const shareProfile = (contactId: string): Promise<void> =>
  invoke('share_profile', { contactId });

export const frontendReady = (): Promise<void> => invoke('frontend_ready');
