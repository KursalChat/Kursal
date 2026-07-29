<script lang="ts">
  import '../app.css';
  import { log } from '$lib/utils/log';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { listen } from '@tauri-apps/api/event';
  import { goto } from '$app/navigation';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { messagesState } from '$lib/state/messages.svelte';
  import { winstonTips } from '$lib/state/winstonTips.svelte';
  import { draftsState } from '$lib/state/drafts.svelte';
  import { pinnedConvosState } from '$lib/state/pinnedConvos.svelte';
  import { archivedConvosState } from '$lib/state/archivedConvos.svelte';
  import { nearbyState } from '$lib/state/nearby.svelte';
  import { typingState } from '$lib/state/typing.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { t } from '$lib/i18n';
  import { appearanceState } from '$lib/state/appearance.svelte';
  import { prefsState } from '$lib/state/prefs.svelte';
  import { settingsState } from '$lib/state/settings.svelte';
  import { notifyMessage, getPermission, isInDndWindow } from '$lib/api/system-notify';
  import { getNetworkStatus } from '$lib/api/settings';
  import { frontendReady } from '$lib/api/identity';
  import { OS, isMobile } from '$lib/api/window';
  import { acceptFileOffer } from '$lib/api/messages';
  import { notifyError } from '$lib/utils/errors';
  import {
    handleBackendDialog,
    runStartupDialogs,
    type BackendDialogPayload,
  } from '$lib/api/dialog-bridge';
  import ToastContainer from '$lib/components/ToastContainer.svelte';
  import ConfirmDialog from '$lib/components/ConfirmDialog.svelte';
  import { confirmDialog, confirmDialogWithCheckbox } from '$lib/state/confirm.svelte';
  import { callState } from '$lib/state/call.svelte';
  import {
    setBusyState,
    closeToBackground,
    closeForceQuit,
    setBackgroundMode,
    setCloseExplainerPending,
  } from '$lib/api/settings';
  import BiometricLock from '$lib/components/BiometricLock.svelte';
  import CloseExplainer from '$lib/components/CloseExplainer.svelte';
  import type {
    MessageReceivedPayload,
    ConnectionChangedPayload,
    NearbyRequestPayload,
    ContactResponse,
    MessageEditedPayload,
    MessageDeletedPayload,
    MessageQueuedOfflinePayload,
    MessagesReadPayload,
    ReactionChangedPayload,
    FileOfferedPayload,
    FileTransferProgressPayload,
    FileReceivedPayload,
    FileTransferFailedPayload,
    TypingIndicatorPayload,
    BackendSignalPayload,
    NetworkOnlinePayload,
    OfflineBundlePublishedPayload,
    OfflineGapSkippedPayload,
    OfflineQueueDrainedPayload,
    OfflineSyncPayload,
    CloseRequestedPayload,
    ContactTerminatedPayload,
  } from '$lib/types';
  import { networkState } from '$lib/state/network.svelte';
  import { offlineSyncState } from '$lib/state/offlineSync.svelte';
  import { initAndroidInsets } from '$lib/utils/android-insets';

  // Runs before mount so the safe-area vars are correct on first paint.
  initAndroidInsets();

  let { children } = $props();
  let backgroundUnread = 0;
  let baseTitle = 'Kursal';

  async function handleAddNodeLink(payload: string) {
    let addr = payload;
    try {
      addr = decodeURIComponent(payload);
    } catch {
      addr = payload;
    }
    addr = addr.trim();
    if (!addr) return;
    const ok = await confirmDialog({
      title: t('settings.network.addNodeConfirmTitle'),
      message: t('settings.network.addNodeConfirmMessage', { addr }),
      confirmLabel: t('settings.network.addNodeConfirmYes'),
    });
    if (!ok) return;
    try {
      await settingsState.addNode(addr);
      notifications.push(t('settings.network.successNodeAdded'), 'success');
    } catch (e) {
      notifyError(e);
    }
  }

  function readAppLockPref(): boolean {
    if (typeof localStorage === 'undefined') return false;
    try {
      return JSON.parse(localStorage.getItem('kursal_app_lock_biometric') ?? 'false');
    } catch {
      return false;
    }
  }
  let unlocked = $state(!isMobile || !readAppLockPref());

  function refreshTitle() {
    document.title = backgroundUnread > 0 ? `(${backgroundUnread}) ${baseTitle}` : baseTitle;
  }

  // Mirror "is something in flight" down to the backend, which needs it on the
  // synchronous close path where it cannot ask the webview first.
  $effect(() => {
    const callActive = callState.status !== 'idle';
    const transferActive = messagesState.hasActiveTransfers;
    void setBusyState(callActive, transferActive).catch(() => {});
  });

  // First close ever: the window would vanish into the tray with no explanation,
  // so the backend blocks it once and lets Winston say what is happening. The
  // answer becomes the setting; the seen flag is mirrored to the backend, which
  // decides on the close path and cannot read localStorage.
  const CLOSE_EXPLAINER_KEY = 'kursal_close_explainer_seen';
  let closeExplainerOpen = $state(false);

  function closeExplainerSeen(): boolean {
    if (typeof localStorage === 'undefined') return true;
    return localStorage.getItem(CLOSE_EXPLAINER_KEY) === 'done';
  }

  // Awaited by both handlers: they destroy the webview right after, which would
  // otherwise race the flag on its way to the backend.
  async function markCloseExplainerSeen() {
    try {
      localStorage.setItem(CLOSE_EXPLAINER_KEY, 'done');
    } catch {
      /* private mode: worst case the explainer runs once more */
    }
    await setCloseExplainerPending(false).catch(() => {});
  }

  async function keepRunningInTray() {
    closeExplainerOpen = false;
    await markCloseExplainerSeen();
    await closeToBackground(false);
  }

  async function quitInsteadOfTray() {
    closeExplainerOpen = false;
    await markCloseExplainerSeen();
    try {
      await setBackgroundMode(false);
    } catch (e) {
      log.error('Failed to disable background mode', e);
    }
    await closeForceQuit();
  }

  // Dismissing without choosing just cancels the close: the window stays and the
  // explainer stays pending for the next attempt.
  function cancelCloseExplainer() {
    closeExplainerOpen = false;
  }

  // A call holds the camera, which cannot survive the window going away, so the
  // only choices are to stay or to hang up. Transfers are backend-only and can
  // finish with the window gone, so they also get the "keep going" option.
  async function handleCloseRequest(payload: CloseRequestedPayload) {
    if (payload.callActive) {
      const ok = await confirmDialog({
        title: t('layout.closeCallTitle'),
        message: payload.transferActive
          ? t('layout.closeCallWithTransferMessage')
          : t('layout.closeCallMessage'),
        confirmLabel: t('layout.closeCallConfirm'),
        tone: 'danger',
      });
      if (!ok) return;
      await callState.end();
      await closeForceQuit();
      return;
    }

    if (payload.transferActive) {
      const { confirmed, checked } = await confirmDialogWithCheckbox({
        title: t('layout.closeTransferTitle'),
        message: t('layout.closeTransferMessage'),
        confirmLabel: t('layout.closeTransferConfirm'),
        tone: 'warning',
        checkbox: { label: t('layout.closeTransferKeepGoing'), defaultChecked: true },
      });
      if (!confirmed) return;
      if (checked) await closeToBackground(true);
      else await closeForceQuit();
      return;
    }

    // A busy prompt above takes priority and leaves the explainer pending, so it
    // still runs on the next close.
    if (payload.firstClose) closeExplainerOpen = true;
  }

  onMount(() => {
    if (OS === 'macos') document.documentElement.classList.add('mac');
    appearanceState.init();
    prefsState.init();
    void settingsState.load();
    void draftsState.init();
    void pinnedConvosState.init();
    void archivedConvosState.init();
    void getPermission();
    // Re-pushed on every mount, including the one after a close-to-tray, so the
    // backend never asks twice.
    if (!isMobile) void setCloseExplainerPending(!closeExplainerSeen()).catch(() => {});
    const unlistenPromises: Array<Promise<() => void>> = [];
    baseTitle = document.title || 'Kursal';

    // Backend-originated dialogs (crash report, updates, file open) render
    // through the in-app ConfirmDialog. Run startup checks only once the
    // listener is subscribed so the first emit is not missed.
    const backendDialogReady = listen<BackendDialogPayload>('backend_dialog', (event) => {
      void handleBackendDialog(event.payload);
    });
    unlistenPromises.push(backendDialogReady);
    void backendDialogReady.then(() => runStartupDialogs());

    const handleVisibilityChange = () => {
      if (!document.hidden && backgroundUnread > 0) {
        backgroundUnread = 0;
        refreshTitle();
      }
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);

    // The backend owns the close decision - it blocks the close itself and asks
    // here only when a call or transfer is live. A previous version guarded this
    // from the webview, but the Rust CloseRequested handler quit the process
    // regardless, so the confirmation never had a chance to run.
    let closeConfirmOpen = false;
    unlistenPromises.push(
      listen<CloseRequestedPayload>('close_requested', async (event) => {
        if (closeConfirmOpen) return;
        closeConfirmOpen = true;
        try {
          await handleCloseRequest(event.payload);
        } finally {
          closeConfirmOpen = false;
        }
      })
    );

    // Mobile keyboard handling: shrink the document to visualViewport.height
    // so content reflows above the keyboard. Listening only to visualViewport
    // events (not focusin/focusout) avoids a jitter where focus fires before
    // the keyboard actually opens, causing two layout passes.
    const syncViewport = () => {
      const vv = window.visualViewport;
      if (vv) {
        document.documentElement.style.setProperty('--app-height', `${vv.height}px`);
      }
      window.scrollTo(0, 0);
    };

    // Resize signals (keyboard open/close, rotation, window resize) can fire
    // while the engine is still reflowing, so one snapshot may latch stale
    // dimensions. Instead of trusting it, re-sync each frame until the
    // reported size holds still; the deadline is a safety cap only.
    let settleRaf = 0;
    const syncUntilStable = () => {
      cancelAnimationFrame(settleRaf);
      const deadline = performance.now() + 600;
      const STABLE_FRAMES = 3;
      let lastW = -1;
      let lastH = -1;
      let stable = 0;
      const step = () => {
        const vv = window.visualViewport;
        const w = vv?.width ?? window.innerWidth;
        const h = vv?.height ?? window.innerHeight;
        syncViewport();
        stable = w === lastW && h === lastH ? stable + 1 : 0;
        lastW = w;
        lastH = h;
        if (stable < STABLE_FRAMES && performance.now() < deadline) {
          settleRaf = requestAnimationFrame(step);
        }
      };
      step();
    };
    syncUntilStable();
    window.visualViewport?.addEventListener('resize', syncUntilStable);
    window.visualViewport?.addEventListener('scroll', syncViewport);
    window.addEventListener('resize', syncUntilStable);

    // Belt-and-suspenders pinch-zoom block for iOS Safari/WKWebView,
    // which still honors gesture events even with user-scalable=no.
    const blockGesture = (e: Event) => e.preventDefault();
    document.addEventListener('gesturestart', blockGesture);
    document.addEventListener('gesturechange', blockGesture);
    document.addEventListener('gestureend', blockGesture);

    // Block double-tap zoom (iOS).
    let lastTouchEnd = 0;
    const blockDoubleTap = (e: TouchEvent) => {
      const now = Date.now();
      if (now - lastTouchEnd <= 350) e.preventDefault();
      lastTouchEnd = now;
    };
    document.addEventListener('touchend', blockDoubleTap, { passive: false });

    void contactsState.load();

    // Seed from a direct query: the network_online event only fires on
    // changes, so a webview (re)load would otherwise show stale defaults.
    void getNetworkStatus()
      .then((s) => networkState.set(s.peerCount > 0, s.peerCount))
      .catch(() => {});

    // frontend_ready replays cold-start deep links as backend_signal, so the
    // listener must be live before we call it or the first emit is missed.
    const backendSignalReady = listen<BackendSignalPayload>('backend_signal', (event) => {
      const signal = event.payload.signal;
      const payload = event.payload.payload;

      if (signal == 'open_settings') {
        goto('/settings');
      } else if (signal == 'new_contact') {
        goto('/add-contact');
      } else if (signal == 'open_chat') {
        if (payload) goto(`/chat/${payload}`);
      } else if (signal == 'open_otp') {
        goto(`/add-contact/otp?receive=${encodeURIComponent(payload)}`);
      } else if (signal == 'add_node') {
        void handleAddNodeLink(payload);
      } else if (signal == 'handle_incoming_error') {
        // Raw Rust error strings are not user-facing copy; the backend already
        // drops the benign cases (unknown/removed peer) silently.
        notifications.push(t('layout.incomingError'), 'error');
      }
    });
    unlistenPromises.push(backendSignalReady);
    void backendSignalReady.then(() => frontendReady());

    // Listen to message_received event
    unlistenPromises.push(
      listen<MessageReceivedPayload>('message_received', (event) => {
        const payload = event.payload;
        payload.timestamp = payload.timestamp * 1000; // Rust gives seconds, UI expects ms
        payload.receivedTimestamp = payload.receivedTimestamp * 1000;
        messagesState.append(payload);
        typingState.clear(payload.contactId);
        // Call records render as call lines in chat and must not raise
        // unread/notifications.
        const isCallRecord = !!payload.callDetails;
        if (isCallRecord) return;
        messagesState.setFirstUnread(payload.contactId, payload.id);
        if (document.hidden) {
          backgroundUnread += 1;
          refreshTitle();
        }
        if (!payload.viaOffline) contactsState.touchLastSeen(payload.contactId);
        const onThisChat = !document.hidden && $page.url.pathname === `/chat/${payload.contactId}`;
        if (!onThisChat && !contactsState.isMuted(payload.contactId)) {
          const name =
            contactsState.getById(payload.contactId)?.displayName ??
            t('notifications.unknownSender');
          if (document.hidden) {
            void notifyMessage({ senderName: name, body: payload.content });
          } else if (prefsState.notificationPreview !== 'none' && !isInDndWindow()) {
            // Window visible but another chat is open: an OS banner can't
            // deep-link on desktop, an in-app toast can.
            notifications.push(t('notifications.newMessageFrom', { sender: name }), 'info', {
              action: {
                label: t('notifications.openChat'),
                onClick: () => goto(`/chat/${payload.contactId}`),
              },
            });
          }
        }
      })
    );

    // Listen to connection_changed event
    unlistenPromises.push(
      listen<ConnectionChangedPayload>('connection_changed', (event) => {
        contactsState.setConnectionStatus(event.payload.contactId, event.payload.status);
      })
    );

    // Listen to network_online - local node has any libp2p peer (relay/bootstrap/contact)
    unlistenPromises.push(
      listen<NetworkOnlinePayload>('network_online', (event) => {
        networkState.set(event.payload.online, event.payload.peerCount);
      })
    );

    // Listen to offline_bundle_published - DHT put dispatched for these msgs
    unlistenPromises.push(
      listen<OfflineBundlePublishedPayload>('offline_bundle_published', (event) => {
        messagesState.markBundlePublished(event.payload.contactId, event.payload.messageIds);
      })
    );

    // Listen to message_failed - offline retry window elapsed for these msgs
    unlistenPromises.push(
      listen<{ contactId: string; messageIds: string[] }>('message_failed', (event) => {
        messagesState.markFailed(event.payload.contactId, event.payload.messageIds);
      })
    );

    // Listen to contact_terminated - the peer removed us (or re-established
    // contact). The chat stays readable; only sending is closed off.
    unlistenPromises.push(
      listen<ContactTerminatedPayload>('contact_terminated', (event) => {
        contactsState.setTerminated(event.payload.contactId, event.payload.terminated);
      })
    );

    // Listen to offline_queue_drained - the backend handed the whole queued
    // batch to the swarm, so "waiting to sync" markers can finally clear. Lives
    // here rather than in the chat route so conversations you haven't opened
    // clear too.
    unlistenPromises.push(
      listen<OfflineQueueDrainedPayload>('offline_queue_drained', (event) => {
        messagesState.flushPendingSync(event.payload.contactId);
      })
    );

    // Listen to offline_sync - the core's 5-minute mailbox poll started or
    // finished, so the UI can say whether it's checking and when it last did.
    unlistenPromises.push(
      listen<OfflineSyncPayload>('offline_sync', (event) => {
        offlineSyncState.setActive(event.payload.active);
      })
    );

    // Listen to offline_gap_skipped - backend gave up waiting on a suppressed
    // offline mailbox counter; surface a non-alarming notice in that chat.
    unlistenPromises.push(
      listen<OfflineGapSkippedPayload>('offline_gap_skipped', (event) => {
        messagesState.addGapNotice(event.payload.contactId, event.payload.counter);
      })
    );

    // Listen to contact_added event
    unlistenPromises.push(
      listen<ContactResponse>('contact_added', (event) => {
        contactsState.upsert(event.payload);
        notifications.push(t('layout.contactAdded'), 'success');
        goto('/chat/' + event.payload.userId);
      })
    );

    // Listen to delivery_confirmed event
    unlistenPromises.push(
      listen<{ messageId: string; contactId: string }>('delivery_confirmed', (event) => {
        messagesState.updateStatus(event.payload.messageId, event.payload.contactId, 'delivered');
      })
    );

    unlistenPromises.push(
      listen<MessagesReadPayload>('messages_read', (event) => {
        messagesState.markMessagesRead(event.payload.contactId, event.payload.messageIds);
      })
    );

    // Backend took the offline path (peer unreachable / no receipt in 60s);
    // it keeps retrying via the DHT. This event can beat the send_text reply
    // that swaps the optimistic UUID - updateStatusIfSending buffers for that.
    unlistenPromises.push(
      listen<MessageQueuedOfflinePayload>('message_queued_offline', (event) => {
        messagesState.updateStatusIfSending(
          event.payload.messageId,
          event.payload.contactId,
          'queued'
        );
        winstonTips.show('offlineMessages');
      })
    );

    // Listen to message_edited event
    unlistenPromises.push(
      listen<MessageEditedPayload>('message_edited', (event) => {
        messagesState.updateContent(
          event.payload.messageId,
          event.payload.contactId,
          event.payload.newContent
        );
      })
    );

    // Listen to message_deleted event
    unlistenPromises.push(
      listen<MessageDeletedPayload>('message_deleted', (event) => {
        messagesState.markDeleted(event.payload.messageId, event.payload.contactId);
      })
    );

    // Listen to message_pinned event
    unlistenPromises.push(
      listen<{ contactId: string; messageId: string; pinned: boolean }>(
        'message_pinned',
        (event) => {
          messagesState.applyPinned(
            event.payload.contactId,
            event.payload.messageId,
            event.payload.pinned
          );
        }
      )
    );

    // Listen to reaction_added event
    unlistenPromises.push(
      listen<ReactionChangedPayload>('reaction_added', (event) => {
        messagesState.addReaction(
          event.payload.messageId,
          event.payload.contactId,
          event.payload.emoji,
          event.payload.contactId
        );
      })
    );

    // Listen to reaction_removed event
    unlistenPromises.push(
      listen<ReactionChangedPayload>('reaction_removed', (event) => {
        messagesState.removeReaction(
          event.payload.messageId,
          event.payload.contactId,
          event.payload.emoji,
          event.payload.contactId
        );
      })
    );

    // Listen to contact_updated event (Either peer ID rotated, OR profile updated)
    unlistenPromises.push(
      listen<ContactResponse>('contact_updated', (event) => {
        const payload = event.payload;
        const existing = contactsState.getById(payload.userId);

        // Save old values to compare, because upsert modifies Svelte 5 state in-place
        const oldPeerId = existing?.peerId;
        const oldName = existing?.displayName;
        const oldAvatar = existing?.avatarBase64;

        // Upsert handles avatar base64 decoding automatically
        contactsState.upsert(payload);

        if (!existing) return;

        const newContact = contactsState.getById(payload.userId);
        if (!newContact) return;

        if (oldPeerId && oldPeerId !== newContact.peerId) {
          notifications.push(t('layout.peerIdRotated', { name: newContact.displayName }), 'info');
        }

        const nameChanged = oldName && oldName !== newContact.displayName;
        const avatarChanged = oldAvatar !== newContact.avatarBase64 && newContact.avatarBase64;

        if (nameChanged && avatarChanged) {
          notifications.push(
            t('layout.contactRenamedAndAvatar', {
              oldName,
              newName: newContact.displayName,
            }),
            'info'
          );
        } else if (nameChanged) {
          notifications.push(
            t('layout.contactRenamed', { oldName, newName: newContact.displayName }),
            'info'
          );
        } else if (avatarChanged) {
          notifications.push(
            t('layout.contactAvatarUpdated', { name: newContact.displayName }),
            'info'
          );
        }
      })
    );

    // Listen to file_offered event
    unlistenPromises.push(
      listen<FileOfferedPayload>('file_offered', async (event) => {
        const payload = event.payload;
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

        const senderName =
          contactsState.getById(payload.contactId)?.displayName ?? t('notifications.unknownSender');
        const onThisChat = !document.hidden && $page.url.pathname === `/chat/${payload.contactId}`;
        if (!onThisChat) {
          void notifyMessage({
            senderName,
            body: t('notifications.sentFile', { filename: payload.filename }),
          });
        }

        if (payload.autodownload) {
          try {
            await acceptFileOffer(payload.contactId, payload.offerId, payload.autodownload);
          } catch (e) {
            messagesState.setAutodownloadPath(payload.offerId, payload.contactId, null);
            notifyError(e, 'fileTransfer.errorAutoDownload');
          }
        }
      })
    );

    // Listen to file transfer progress
    unlistenPromises.push(
      listen<FileTransferProgressPayload>('file_transfer_progress', (event) => {
        const { transferId, bytesTransferred, totalBytes } = event.payload;
        messagesState.setTransferProgress(transferId, bytesTransferred, totalBytes);
      })
    );

    // Listen to file_received event
    unlistenPromises.push(
      listen<FileReceivedPayload>('file_received', async (event) => {
        const { contactId, transferId, savePath } = event.payload;

        // The bubble's <img> was mounted against the still-empty preallocated
        // file, so the completed bytes only show after a forced refetch.
        messagesState.markMediaReady(contactId, transferId);
        messagesState.clearTransferProgress(transferId);

        // savePath is where the bytes actually landed, which beats the path
        // resolved at accept time (a retry can have changed it).
        messagesState.setAutodownloadPath(transferId, contactId, savePath);
      })
    );

    // Listen to file_transfer_failed event
    unlistenPromises.push(
      listen<FileTransferFailedPayload>('file_transfer_failed', (event) => {
        const { transferId, reason } = event.payload;
        messagesState.clearTransferProgress(transferId);
        // A cancel is user-initiated and the bubble reverts on its own.
        if (reason !== 'cancelled') {
          notifications.push(t('fileTransfer.transferFailed'), 'error');
        }
        log.error('File transfer failed', event.payload);
      })
    );

    // Listen to nearby_request event
    unlistenPromises.push(
      listen<NearbyRequestPayload>('nearby_request', (event) => {
        nearbyState.addPendingRequest(event.payload.peerId, event.payload.sessionName);
      })
    );

    // Listen to typing_indicator event
    unlistenPromises.push(
      listen<TypingIndicatorPayload>('typing_indicator', (event) => {
        typingState.set(event.payload.contactId, event.payload.replyTo ?? null);
      })
    );

    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      cancelAnimationFrame(settleRaf);
      window.visualViewport?.removeEventListener('resize', syncUntilStable);
      window.visualViewport?.removeEventListener('scroll', syncViewport);
      window.removeEventListener('resize', syncUntilStable);
      document.removeEventListener('gesturestart', blockGesture);
      document.removeEventListener('gesturechange', blockGesture);
      document.removeEventListener('gestureend', blockGesture);
      document.removeEventListener('touchend', blockDoubleTap);
      backgroundUnread = 0;
      refreshTitle();
      void Promise.all(unlistenPromises).then((fns) => {
        fns.forEach((fn) => fn());
      });
    };
  });
</script>

{#if unlocked}
  {@render children()}
{:else}
  <BiometricLock onUnlock={() => (unlocked = true)} />
{/if}
<ToastContainer />
<ConfirmDialog />
<CloseExplainer
  open={closeExplainerOpen}
  onKeep={() => void keepRunningInTray()}
  onOptOut={() => void quitInsteadOfTray()}
  onCancel={cancelCloseExplainer}
/>
