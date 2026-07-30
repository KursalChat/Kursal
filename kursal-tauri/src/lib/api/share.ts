import { invoke } from '@tauri-apps/api/core';
import type { SharePayload } from '$lib/types';

// Drains the OS drop-box: returns queued share payloads and forgets them.
export const takePendingShares = (): Promise<SharePayload[]> => invoke('take_pending_shares');

export const discardPendingShare = (id: string): Promise<void> =>
  invoke('discard_pending_share', { id });
