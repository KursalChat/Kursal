import { invoke } from '@tauri-apps/api/core';
import type { PendingSyncSnapshot, UnreadEntry } from '$lib/types';

/** Unread state for every contact, derived from each one's stored read cursor. */
export const getUnreadSummary = (): Promise<UnreadEntry[]> => invoke('get_unread_summary');

/** Returns the ids that just became read, ready to send as read receipts. */
export const markContactRead = (contactId: string): Promise<string[]> =>
  invoke('mark_contact_read', { contactId });

export const markContactUnread = (
  contactId: string,
  fromMessageId: string | null = null
): Promise<UnreadEntry> => invoke('mark_contact_unread', { contactId, fromMessageId });

export const setContactMarkedUnread = (contactId: string, value: boolean): Promise<void> =>
  invoke('set_contact_marked_unread', { contactId, value });

export const getDelayedUnseen = (): Promise<Record<string, string[]>> =>
  invoke('get_delayed_unseen');

export const setDelayedUnseen = (contactId: string, messageIds: string[]): Promise<void> =>
  invoke('set_delayed_unseen', { contactId, messageIds });

export const getPendingSync = (): Promise<PendingSyncSnapshot> => invoke('get_pending_sync');
