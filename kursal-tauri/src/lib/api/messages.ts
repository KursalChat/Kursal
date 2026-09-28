import { invoke } from '@tauri-apps/api/core';
import type { MessageResponse } from '$lib/types';

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

/** Returns [messageId, sizeBytes, storedPath]: storedPath is the path the core kept the copy at. */
export const sendFileOffer = (
  contactId: string,
  filePath: string,
  allowUnstripped = false
): Promise<[string, number, string]> =>
  invoke('send_file_offer', { contactId, filePath, allowUnstripped });

export const createOutgoingPendingPath = (filename: string): Promise<string> =>
  invoke('create_outgoing_pending_path', { filename });

export const acceptFileOffer = (
  contactId: string,
  offerId: string,
  savePath: string
): Promise<void> => invoke('accept_file_offer', { contactId, offerId, savePath });

export const resolveDownloadPath = (
  contactId: string,
  offerId: string,
  filename: string
): Promise<string> => invoke('resolve_download_path', { contactId, offerId, filename });

export const availableSpace = (path: string): Promise<number | null> =>
  invoke('available_space', { path });

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
