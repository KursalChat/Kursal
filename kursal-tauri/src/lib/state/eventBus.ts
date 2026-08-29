import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { contactsState } from '$lib/state/contacts.svelte';
import { messagesState } from '$lib/state/messages.svelte';
import { networkState } from '$lib/state/network.svelte';
import { nearbyState } from '$lib/state/nearby.svelte';
import { typingState } from '$lib/state/typing.svelte';
import { offlineSyncState } from '$lib/state/offlineSync.svelte';
import { updateDownloadState } from '$lib/state/updateDownload.svelte';
import { winstonTips } from '$lib/state/winstonTips.svelte';
import { notifications } from '$lib/state/notifications.svelte';
import { acceptFileOffer } from '$lib/api/messages';
import { notifyError, parseError } from '$lib/utils/errors';
import { clearOtpSession } from '$lib/utils/otpSession';
import { log } from '$lib/utils/log';
import { t } from '$lib/i18n';
import type {
  MessageReceivedPayload,
  ConnectionChangedPayload,
  NearbyRequestPayload,
  ContactResponse,
  ContactTerminatedPayload,
  MessageEditedPayload,
  MessageDeletedPayload,
  MessageQueuedOfflinePayload,
  MessagesReadPayload,
  NetworkOnlinePayload,
  OfflineBundlePublishedPayload,
  OfflineGapSkippedPayload,
  OfflineQueueDrainedPayload,
  OfflineSyncPayload,
  ReactionChangedPayload,
  FileOfferedPayload,
  FileReceivedPayload,
  FileTransferFailedPayload,
  FileTransferProgressPayload,
  TypingIndicatorPayload,
  UpdateDownloadProgressPayload,
} from '$lib/types';

export interface CoreListenerHooks {
  notifyIncoming: (contactId: string, senderName: string, body: string) => void;
  onContactAdded: (contact: ContactResponse) => void;
}

function senderName(contactId: string): string {
  return contactsState.getById(contactId)?.displayName ?? t('notifications.unknownSender');
}

// Maps every core AppEvent onto its store. Returns one teardown for the lot.
export function registerCoreListeners(hooks: CoreListenerHooks): () => void {
  const pending: Array<Promise<UnlistenFn>> = [];
  const on = <T>(event: string, handler: (payload: T) => void) => {
    pending.push(listen<T>(event, (e) => handler(e.payload)));
  };

  on<MessageReceivedPayload>('message_received', (payload) => {
    payload.timestamp = payload.timestamp * 1000; // Rust gives seconds, UI expects ms
    payload.receivedTimestamp = payload.receivedTimestamp * 1000;
    contactsState.touchLastMessage(payload.contactId, payload.timestamp);
    messagesState.append(payload);
    typingState.clear(payload.contactId);
    // Call records render as call lines in chat and must not raise
    // unread/notifications. Neither can a record of my own action: pinning
    // comes back through this event with direction 'sent'.
    if (payload.callDetails || payload.direction === 'sent') return;
    messagesState.setFirstUnread(payload.contactId, payload.id);
    if (!payload.viaOffline) contactsState.touchLastSeen(payload.contactId);
    hooks.notifyIncoming(payload.contactId, senderName(payload.contactId), payload.content);
  });

  on<ConnectionChangedPayload>('connection_changed', (p) => {
    contactsState.setConnectionStatus(p.contactId, p.status);
  });

  // "Online" means the node has any libp2p peer: relay, bootstrap, or contact.
  on<NetworkOnlinePayload>('network_online', (p) => {
    networkState.set(p.online, p.peerCount);
  });

  // The DHT put for these messages has already been dispatched.
  on<OfflineBundlePublishedPayload>('offline_bundle_published', (p) => {
    messagesState.markBundlePublished(p.contactId, p.messageIds);
  });

  // The offline retry window elapsed for these messages.
  on<{ contactId: string; messageIds: string[] }>('message_failed', (p) => {
    messagesState.markFailed(p.contactId, p.messageIds);
  });

  // The peer removed us, or we re-established contact. The chat stays
  // readable; only sending is closed off.
  on<ContactTerminatedPayload>('contact_terminated', (p) => {
    contactsState.setTerminated(p.contactId, p.terminated);
  });

  on<OfflineQueueDrainedPayload>('offline_queue_drained', (p) => {
    messagesState.flushPendingSync(p.contactId, p.finalizedDeletes ?? []);
  });

  on<OfflineSyncPayload>('offline_sync', (p) => {
    offlineSyncState.setActive(p.active);
  });

  on<OfflineGapSkippedPayload>('offline_gap_skipped', (p) => {
    messagesState.addGapNotice(p.contactId, p.counter);
  });

  on<ContactResponse>('contact_added', (p) => {
    contactsState.upsert(p);
    hooks.onContactAdded(p);
  });

  // The OTP is single-use, so this session data is now dead and can be cleared.
  on('otp_consumed', () => clearOtpSession());

  on<{ messageId: string; contactId: string }>('delivery_confirmed', (p) => {
    messagesState.updateStatus(p.messageId, p.contactId, 'delivered');
  });

  on<MessagesReadPayload>('messages_read', (p) => {
    messagesState.markMessagesRead(p.contactId, p.messageIds);
  });

  on<MessageQueuedOfflinePayload>('message_queued_offline', (p) => {
    messagesState.updateStatusIfSending(p.messageId, p.contactId, 'queued');
    winstonTips.show('offlineMessages');
  });

  on<MessageEditedPayload>('message_edited', (p) => {
    messagesState.updateContent(p.messageId, p.contactId, p.newContent);
  });

  on<MessageDeletedPayload>('message_deleted', (p) => {
    messagesState.markDeleted(p.messageId, p.contactId);
  });

  on<{ contactId: string; messageId: string; pinned: boolean }>('message_pinned', (p) => {
    messagesState.applyPinned(p.contactId, p.messageId, p.pinned);
  });

  on<ReactionChangedPayload>('reaction_added', (p) => {
    messagesState.addReaction(p.messageId, p.contactId, p.emoji, p.contactId);
  });

  on<ReactionChangedPayload>('reaction_removed', (p) => {
    messagesState.removeReaction(p.messageId, p.contactId, p.emoji, p.contactId);
  });

  on<ContactResponse>('contact_updated', (payload) => {
    const existing = contactsState.getById(payload.userId);

    // Saved before upsert, which mutates the Svelte 5 state object in place.
    const oldPeerId = existing?.peerId;
    const oldName = existing?.displayName;
    const oldAvatar = existing?.avatarPath;

    contactsState.upsert(payload);
    if (!existing) return;

    const next = contactsState.getById(payload.userId);
    if (!next) return;

    if (oldPeerId && oldPeerId !== next.peerId) {
      notifications.push(t('layout.peerIdRotated', { name: next.displayName }), 'info');
    }

    const nameChanged = oldName && oldName !== next.displayName;
    const avatarChanged = oldAvatar !== next.avatarPath && next.avatarPath;

    if (nameChanged && avatarChanged) {
      notifications.push(
        t('layout.contactRenamedAndAvatar', { oldName, newName: next.displayName }),
        'info'
      );
    } else if (nameChanged) {
      notifications.push(
        t('layout.contactRenamed', { oldName, newName: next.displayName }),
        'info'
      );
    } else if (avatarChanged) {
      notifications.push(t('layout.contactAvatarUpdated', { name: next.displayName }), 'info');
    }
  });

  on<FileOfferedPayload>('file_offered', async (payload) => {
    messagesState.append({
      id: payload.offerId,
      contactId: payload.contactId,
      direction: 'received',
      content: t('chat.conversation.filePlaceholder'),
      status: 'delivered',
      timestamp: Date.now(),
      receivedTimestamp: Date.now(),
      replyTo: null,
      fileDetails: {
        filename: payload.filename,
        sizeBytes: payload.sizeBytes,
        autodownloadPath: payload.autodownload,
      },
    });
    messagesState.setFirstUnread(payload.contactId, payload.offerId);

    hooks.notifyIncoming(
      payload.contactId,
      senderName(payload.contactId),
      t('notifications.sentFile', { filename: payload.filename })
    );

    if (!payload.autodownload) return;
    try {
      await acceptFileOffer(payload.contactId, payload.offerId, payload.autodownload);
    } catch (e) {
      messagesState.setAutodownloadPath(payload.offerId, payload.contactId, null);
      notifyError(
        e,
        parseError(e).code === 'insufficient_space'
          ? 'fileTransfer.errorNoSpace'
          : 'fileTransfer.errorAutoDownload'
      );
    }
  });

  on<FileTransferProgressPayload>('file_transfer_progress', (p) => {
    messagesState.setTransferProgress(p.transferId, p.bytesTransferred, p.totalBytes);
  });

  on<FileReceivedPayload>('file_received', (p) => {
    // The bubble's <img> was mounted against the still-empty preallocated
    // file, so the completed bytes only show after a forced refetch.
    messagesState.markMediaReady(p.contactId, p.transferId);
    messagesState.clearTransferProgress(p.transferId);

    // savePath is where the bytes actually landed, which beats the path
    // resolved at accept time (a retry can have changed it).
    messagesState.setAutodownloadPath(p.transferId, p.contactId, p.savePath);
  });

  on<FileTransferFailedPayload>('file_transfer_failed', (p) => {
    messagesState.clearTransferProgress(p.transferId);
    // A cancel is user-initiated and the bubble reverts on its own.
    if (p.reason !== 'cancelled') {
      notifications.push(t('fileTransfer.transferFailed'), 'error');
    }
    log.error('File transfer failed', p);
  });

  on<NearbyRequestPayload>('nearby_request', (p) => {
    nearbyState.addPendingRequest(p.peerId, p.sessionName);
  });

  on<TypingIndicatorPayload>('typing_indicator', (p) => {
    typingState.set(p.contactId, p.replyTo ?? null);
  });

  on<UpdateDownloadProgressPayload>('update_download_progress', (p) => {
    updateDownloadState.setProgress(p.downloaded, p.contentLength);
  });

  on('update_download_finished', () => updateDownloadState.finish());

  return () => {
    void Promise.all(pending).then((fns) => fns.forEach((fn) => fn()));
  };
}
