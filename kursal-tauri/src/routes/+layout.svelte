<script lang="ts">
  import '../app.css';
  import { log } from '$lib/utils/log';
  import { onMount } from 'svelte';
  import { page } from '$app/stores';
  import { listen } from '@tauri-apps/api/event';
  import { onOpenUrl } from '@tauri-apps/plugin-deep-link';
  import { goto } from '$app/navigation';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { messagesState } from '$lib/state/messages.svelte';
  import { draftsState } from '$lib/state/drafts.svelte';
  import { pinnedConvosState } from '$lib/state/pinnedConvos.svelte';
  import { archivedConvosState } from '$lib/state/archivedConvos.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { t } from '$lib/i18n';
  import { appearanceState } from '$lib/state/appearance.svelte';
  import { prefsState } from '$lib/state/prefs.svelte';
  import { settingsState } from '$lib/state/settings.svelte';
  import { notifyMessage, getPermission, isInDndWindow } from '$lib/state/systemNotify.svelte';
  import { getNetworkStatus } from '$lib/api/settings';
  import { frontendReady } from '$lib/api/identity';
  import { OS, isMobile } from '$lib/api/window';
  import { notifyError } from '$lib/utils/errors';
  import { registerCoreListeners } from '$lib/state/eventBus';
  import { trackViewport, blockPinchZoom } from '$lib/utils/viewport';
  import { readRaw, writeRaw, readJson } from '$lib/utils/storage';
  import { APP_LOCK_KEY } from '$lib/utils/storage-keys';
  import { runStartupDialogs } from '$lib/api/dialogs';
  import { handleBackendDialog } from '$lib/state/dialogBridge.svelte';
  import ToastContainer from '$lib/components/ToastContainer.svelte';
  import UpdateDownloadRing from '$lib/components/UpdateDownloadRing.svelte';
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
  import ShareTargetModal from '$lib/components/ShareTargetModal.svelte';
  import { shareIntentState } from '$lib/state/shareIntent.svelte';
  import CloseExplainer from '$lib/components/CloseExplainer.svelte';
  import type {
    BackendSignalPayload,
    CloseRequestedPayload,
    BackendDialogPayload,
  } from '$lib/types';
  import { networkState } from '$lib/state/network.svelte';
  import { appFocusState } from '$lib/state/appFocus.svelte';
  import { initAndroidInsets } from '$lib/utils/android-insets';

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

  let unlocked = $state(!isMobile || !readJson(APP_LOCK_KEY, false));

  // Registered before the first drain so a payload that finishes staging mid
  // startup still reaches us.
  const stopShareBridge = shareIntentState.listenForNative();

  // Shares staged by the OS are drained once the app is usable, so a locked
  // app keeps the payload on disk instead of dropping it.
  $effect(() => {
    if (unlocked) void shareIntentState.drain();
  });

  function handleSharePick(contactId: string) {
    shareIntentState.assign(contactId);
    void goto(`/chat/${contactId}`);
  }

  function refreshTitle() {
    document.title = backgroundUnread > 0 ? `(${backgroundUnread}) ${baseTitle}` : baseTitle;
  }

  $effect(() => {
    if (appFocusState.focused && backgroundUnread > 0) {
      backgroundUnread = 0;
      refreshTitle();
    }
  });

  // OS banner only when the window is unfocused; focused app gets an in-app
  // toast instead, and nothing at all while the sender's chat is open.
  function notifyIncoming(contactId: string, senderName: string, body: string) {
    const background = !appFocusState.focused;
    if (background) {
      backgroundUnread += 1;
      refreshTitle();
    }
    if (contactsState.isMuted(contactId)) return;
    if (background) {
      void notifyMessage({ contactId, senderName, body });
      return;
    }
    if ($page.url.pathname === `/chat/${contactId}`) return;
    if (prefsState.notificationPreview === 'none' || isInDndWindow()) return;
    notifications.push(t('notifications.newMessageFrom', { sender: senderName }), 'info', {
      action: {
        label: t('notifications.openChat'),
        onClick: () => goto(`/chat/${contactId}`),
      },
    });
  }

  $effect(() => {
    const callActive = callState.status !== 'idle';
    const transferActive = messagesState.hasActiveTransfers;
    void setBusyState(callActive, transferActive).catch(() => {});
  });

  const CLOSE_EXPLAINER_KEY = 'kursal_close_explainer_seen';
  let closeExplainerOpen = $state(false);

  function closeExplainerSeen(): boolean {
    return readRaw(CLOSE_EXPLAINER_KEY) === 'done';
  }

  async function markCloseExplainerSeen() {
    writeRaw(CLOSE_EXPLAINER_KEY, 'done');
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

  function cancelCloseExplainer() {
    closeExplainerOpen = false;
  }

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

    if (!isMobile) void setCloseExplainerPending(!closeExplainerSeen()).catch(() => {});
    const unlistenPromises: Array<Promise<() => void>> = [];
    baseTitle = document.title || 'Kursal';

    const backendDialogReady = listen<BackendDialogPayload>('backend_dialog', (event) => {
      void handleBackendDialog(event.payload);
    });
    unlistenPromises.push(backendDialogReady);
    void backendDialogReady.then(() => runStartupDialogs());

    // The iOS share extension foregrounds the app with kursal://share.
    unlistenPromises.push(onOpenUrl(() => void shareIntentState.drain()));

    // Desktop "Open with Kursal" hands the files straight to the backend.
    unlistenPromises.push(listen('share_received', () => void shareIntentState.drain()));

    const stopFocusTracking = appFocusState.init();

    const handleVisibilityChange = () => {
      if (document.hidden) {
        void draftsState.flush().catch(() => {});
      } else {
        void shareIntentState.drain();
      }
    };
    document.addEventListener('visibilitychange', handleVisibilityChange);

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

    const stopViewportTracking = trackViewport();
    const stopGestureBlock = blockPinchZoom();

    void contactsState.load();
    void messagesState.hydrate();

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
      } else if (signal == 'open_otp') {
        goto(`/add-contact?receive=${encodeURIComponent(payload)}`);
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

    const stopCoreListeners = registerCoreListeners({
      notifyIncoming,
      onContactAdded: (contact) => {
        notifications.push(t('layout.contactAdded'), 'success');
        const verify = contact.viaNearby && !contact.verified;
        goto('/chat/' + contact.userId + (verify ? '?verify=1' : ''));
      },
    });

    return () => {
      document.removeEventListener('visibilitychange', handleVisibilityChange);
      stopShareBridge();
      stopFocusTracking();
      stopViewportTracking();
      stopGestureBlock();
      stopCoreListeners();
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
{#if unlocked && shareIntentState.awaitingTarget && shareIntentState.head}
  <ShareTargetModal
    payload={shareIntentState.head}
    onPick={handleSharePick}
    onCancel={() => void shareIntentState.discardHead()}
  />
{/if}
<ToastContainer />
<UpdateDownloadRing />
<ConfirmDialog />
<CloseExplainer
  open={closeExplainerOpen}
  onKeep={() => void keepRunningInTray()}
  onOptOut={() => void quitInsteadOfTray()}
  onCancel={cancelCloseExplainer}
/>
