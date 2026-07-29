import { invoke } from '@tauri-apps/api/core';
import type { MessageResponse } from '$lib/types';

// The Rust backend stores and returns timestamps in seconds. Convert both the
// sent (`timestamp`) and received (`receivedTimestamp`) times to JS
// milliseconds at the API boundary so the rest of the frontend can rely on ms.
function hydrateTimestamps(m: MessageResponse): MessageResponse {
  m.timestamp = m.timestamp * 1000;
  m.receivedTimestamp = m.receivedTimestamp * 1000;
  return m;
}

export const sendText = (
  contactId: string,
  text: string,
  replyTo: string | null = null
): Promise<string> => invoke('send_text', { contactId, text, replyTo });

export const deleteLocalMessage = (contactId: string, messageId: string): Promise<void> =>
  invoke('delete_local_message', { contactId, messageId });

// The four "change" commands below resolve to true when the backend put the
// change on the offline queue instead of delivering it directly. The UI uses
// that to decide whether to show the "waiting to sync" marker - guessing from
// connection status gets it wrong and leaves the marker stuck forever.
export const deleteMessage = (contactId: string, messageId: string): Promise<boolean> =>
  invoke('delete_message_for_everyone', { contactId, messageId });

export const editMessage = (
  contactId: string,
  messageId: string,
  newContent: string
): Promise<boolean> => invoke('edit_message', { contactId, messageId, newContent });

export const addReaction = (
  contactId: string,
  messageId: string,
  emoji: string
): Promise<boolean> => invoke('add_reaction', { contactId, messageId, emoji });

export const removeReaction = (
  contactId: string,
  messageId: string,
  emoji: string
): Promise<boolean> => invoke('remove_reaction', { contactId, messageId, emoji });

export const pinMessage = (contactId: string, messageId: string, pinned: boolean): Promise<void> =>
  invoke('pin_message', { contactId, messageId, pinned });

export const getPinnedMessages = async (contactId: string): Promise<MessageResponse[]> => {
  const msgs = await invoke<MessageResponse[]>('get_pinned_messages', { contactId });
  return msgs.map(hydrateTimestamps);
};

export const sendFileOffer = (contactId: string, filePath: string): Promise<[string, number]> =>
  invoke('send_file_offer', { contactId, filePath });

export const acceptFileOffer = (
  contactId: string,
  offerId: string,
  savePath: string
): Promise<void> => invoke('accept_file_offer', { contactId, offerId, savePath });

// Destination a download lands in. The core owns this path (it is the same one
// auto-download uses) so the filename sanitising stays in one place.
export const resolveDownloadPath = (
  contactId: string,
  offerId: string,
  filename: string
): Promise<string> => invoke('resolve_download_path', { contactId, offerId, filename });

export const cancelFileTransfer = (contactId: string, offerId: string): Promise<void> =>
  invoke('cancel_file_transfer', { contactId, offerId });

export const sendTypingIndicator = (
  contactId: string,
  replyTo: string | null = null
): Promise<void> => invoke('send_typing_indicator', { contactId, replyTo });

export const flushOffline = (contactId: string): Promise<void> =>
  invoke('flush_offline', { contactId });

export const sendReadReceipts = (contactId: string, messageIds: string[]): Promise<void> =>
  invoke('send_read_receipts', { contactId, messageIds });

export const retryMessage = (contactId: string, messageId: string): Promise<void> =>
  invoke('retry_message', { contactId, messageId });

export const getMessages = async (
  contactId: string,
  limit = 100,
  before: string | null = null
): Promise<MessageResponse[]> => {
  const msgs = await invoke<MessageResponse[]>('get_messages', { contactId, limit, before });
  // The Rust backend stores and returns timestamps in seconds.
  // We convert them to JS milliseconds at the API boundary so the entire frontend
  // can reliably expect 'timestamp' to be in milliseconds (e.g. for new Date()).
  return msgs.map(hydrateTimestamps);
};

export const getMessagesAfter = async (
  contactId: string,
  after: string,
  limit = 50
): Promise<MessageResponse[]> => {
  const msgs = await invoke<MessageResponse[]>('get_messages_after', { contactId, after, limit });
  return msgs.map(hydrateTimestamps);
};

export const getMessagesAround = async (
  contactId: string,
  messageId: string,
  limit = 50
): Promise<MessageResponse[]> => {
  const msgs = await invoke<MessageResponse[]>('get_messages_around', {
    contactId,
    messageId,
    limit,
  });
  return msgs.map(hydrateTimestamps);
};

export const searchMessages = async (
  contactId: string,
  query: string,
  limit = 200
): Promise<MessageResponse[]> => {
  const msgs = await invoke<MessageResponse[]>('search_messages', { contactId, query, limit });
  return msgs.map(hydrateTimestamps);
};

export const searchMessagesGlobal = async (
  query: string,
  limit = 50
): Promise<MessageResponse[]> => {
  const msgs = await invoke<MessageResponse[]>('search_messages_global', { query, limit });
  return msgs.map(hydrateTimestamps);
};
