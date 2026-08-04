import { invoke } from '@tauri-apps/api/core';
import {
  shareFile as sharekitShareFile,
  type SharePosition,
} from '@choochmeque/tauri-plugin-sharekit-api';
import { OS, isMobile } from '$lib/api/window';
import { parseError } from '$lib/utils/errors';
import type { SharePayload } from '$lib/types';

export const takePendingShares = (): Promise<SharePayload[]> => invoke('take_pending_shares');

export const discardPendingShare = (id: string): Promise<void> =>
  invoke('discard_pending_share', { id });

export const canShareFiles = isMobile || OS === 'macos';

export function shareAnchor(el: HTMLElement): SharePosition {
  const rect = el.getBoundingClientRect();
  return {
    x: rect.left + rect.width / 2,
    y: rect.top + rect.height / 2,
    preferredEdge: 'bottom',
  };
}

export async function shareFile(
  path: string,
  mimeType?: string,
  title?: string,
  position?: SharePosition
): Promise<boolean> {
  try {
    const url = OS === 'macos' ? path : `file://${path}`;
    await sharekitShareFile(url, { mimeType, title, position });
    return true;
  } catch (e) {
    if (parseError(e).message.toLowerCase().includes('cancel')) return false;
    throw e;
  }
}
