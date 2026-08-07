<script lang="ts" module>
  // Persists per-contact scroll position across chat switches (module scope
  // survives route param changes). Absent entry => never opened => start at bottom.
  const scrollMemory = new Map<string, number>();

  const UNREAD_TAIL_LIMIT = 5;
</script>

<script lang="ts">
  import { page } from '$app/state';
  import { log } from '$lib/utils/log';
  import { onMount, tick, untrack } from 'svelte';
  import { browser } from '$app/environment';
  import { goto } from '$app/navigation';
  import { stat } from '@tauri-apps/plugin-fs';
  import { t } from '$lib/i18n';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { isOnlineStatus } from '$lib/utils/presence';
  import { messagesState } from '$lib/state/messages.svelte';
  import { profileState } from '$lib/state/profile.svelte';
  import { uiState } from '$lib/state/ui.svelte';
  import { settingsState } from '$lib/state/settings.svelte';
  import { draftsState } from '$lib/state/drafts.svelte';
  import { appearanceState } from '$lib/state/appearance.svelte';
  import { winstonTips } from '$lib/state/winstonTips.svelte';
  import { pendingDropState, contactDropTargetAt } from '$lib/state/pendingDrop.svelte';
  import { shareIntentState } from '$lib/state/shareIntent.svelte';
  import { confirmDialog } from '$lib/state/confirm.svelte';
  import {
    sendText,
    sendFileOffer,
    acceptFileOffer,
    cancelFileTransfer,
    deleteLocalMessage,
    deleteMessage,
    editMessage,
    addReaction,
    removeReaction,
    pinMessage,
    searchMessages,
    sendTypingIndicator,
    flushOffline,
    resolveDownloadPath,
  } from '$lib/api/messages';
  import { shareProfile } from '$lib/api/identity';
  import { isMobile } from '$lib/api/window';
  import {
    pickFilesForSend,
    prepareOfferSourcePath,
    prepareOfferFromFile,
    prepareOfferFromBytes,
    exportToDevice,
  } from '$lib/utils/file-transfer-paths';
  import type { PickerMode } from '$lib/utils/file-transfer-paths';
  import type { MessageResponse, SharePayload } from '$lib/types';
  import { notifications } from '$lib/state/notifications.svelte';
  import { flashSet } from '$lib/utils/flash.svelte';
  import * as haptics from '$lib/utils/haptics';
  import { Paperclip } from 'lucide-svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { busy } from '$lib/utils/busy.svelte';
  import { notifyError, parseError } from '$lib/utils/errors';
  import SecurityCodeModal from '$lib/components/SecurityCodeModal.svelte';
  import ConnectionInfoModal from '$lib/components/ConnectionInfoModal.svelte';
  import ProfileModal from '$lib/components/ProfileModal.svelte';
  import ChatHeader from './ChatHeader.svelte';
  import PinBar from './PinBar.svelte';
  import UnreadBar from './UnreadBar.svelte';
  import ChatSearchBar from './ChatSearchBar.svelte';
  import MessageBubble from './MessageBubble.svelte';
  import MessageComposer from './MessageComposer.svelte';
  import ShareBanner from './ShareBanner.svelte';
  import ScrollToBottomButton from './ScrollToBottomButton.svelte';
  import DelayedMessagesButton from './DelayedMessagesButton.svelte';
  import MessageList from './MessageList.svelte';
  import OfflineQueueBar from './OfflineQueueBar.svelte';
  import ActionSheet from './ActionSheet.svelte';
  import AttachSheet from './AttachSheet.svelte';
  import ForwardModal from './ForwardModal.svelte';
  import SelectTextModal from './SelectTextModal.svelte';
  import FileConfirmModal from './FileConfirmModal.svelte';
  import MediaViewer from './MediaViewer.svelte';
  import EmojiPicker from '$lib/components/EmojiPicker.svelte';
  import {
    getMessagePreview,
    handleMarkdownClick,
    mediaKindFromFilename,
    transferPercent,
    isTransferDone,
    emojiPickerPosition,
    mediaUrl,
    isMessageActionable,
    PICKER_W,
  } from './chat-utils';
  import type { EmojiPickerPos } from './chat-utils';
  import { buildMessageGroups, sortByOfflineTier } from './chat-grouping';

  const shareBusy = busy();

  const contactId = $derived(page.params.id ?? '');
  const contact = $derived(contactId ? contactsState.getById(contactId) : null);
  const messages = $derived(contactId ? messagesState.forContact(contactId) : []);
  const firstUnreadId = $derived(contactId ? messagesState.firstUnreadFor(contactId) : null);
  const pendingUnread = $derived(contactId ? messagesState.unreadFor(contactId) : 0);
  const lastReceivedTs = $derived(
    messages.reduce((ts, m) => (m.direction === 'received' ? Math.max(ts, m.timestamp) : ts), 0)
  );
  // Counted off the separator, not the unread tally: arriving at the bottom
  // clears the tally, but the run it marked is still what you scrolled past.
  const unreadRunCount = $derived.by(() => {
    if (!firstUnreadId) return 0;
    const idx = messages.findIndex((m) => m.id === firstUnreadId);
    if (idx === -1) return pendingUnread;
    let n = 0;
    for (let i = idx; i < messages.length; i++) if (messages[i].direction === 'received') n++;
    return n;
  });
  const unreadFromTs = $derived(
    pendingUnread > 0 && firstUnreadId
      ? (messages.find((m) => m.id === firstUnreadId)?.timestamp ?? null)
      : null
  );

  // Needs a received message at or below the anchor to mean anything, and the
  // anchor itself must not already sit inside the unread run.
  function canMarkUnread(msg: MessageResponse): boolean {
    if (lastReceivedTs === 0 || msg.timestamp > lastReceivedTs) return false;
    return unreadFromTs === null || msg.timestamp < unreadFromTs;
  }
  const gapNotices = $derived(contactId ? messagesState.gapNoticesFor(contactId) : []);

  $effect(() => {
    if (!contact || contact.verified || messages.length < 10) return;
    winstonTips.show('verifyContact', openSecurityCodeModal);
  });

  // Files dropped on this contact's sidebar row while another view was open.
  $effect(() => {
    if (!contactId) return;
    const paths = pendingDropState.consume(contactId);
    if (!paths) return;
    void (async () => {
      try {
        const prepared = await Promise.all(paths.map((p) => prepareOfferSourcePath(p)));
        await stageFilesForSend(prepared);
      } catch (e) {
        notifyError(e, 'chat.conversation.errorPrepareFile');
      }
    })();
  });

  // Only messages created after the chat was opened get the entrance animation;
  // everything loaded from history renders without the pop. Reset per chat.
  let chatOpenedAt = $state(Date.now());
  $effect(() => {
    void contactId;
    chatOpenedAt = Date.now();
  });

  let inputText = $state('');
  let sending = $state(false);
  let showSecurityCode = $state(false);
  let showConnectionInfo = $state(false);
  let showProfile = $state(false);
  let hoveredMessageId = $state<string | null>(null);
  let replyingToMessageId = $state<string | null>(null);
  let editingMessageId = $state<string | null>(null);
  let showEmojiPicker = $state<string | null>(null);
  let emojiPickerAnchor = $state<DOMRect | null>(null);
  let actionSheetMsgId = $state<string | null>(null);
  let forwardContent = $state<string | null>(null);
  let selectTextMsgId = $state<string | null>(null);
  let fileOfferActionState = $state<Record<string, 'idle' | 'accepting' | 'accepted'>>({});
  // Copy and save-to-device run from the action sheet, which closes on click:
  // the bubble itself flashes the confirmation instead.
  const messageFlash = flashSet();
  const completedFileTimers = new Map<string, ReturnType<typeof setTimeout>>();
  let isCoarsePointer = $state(false);
  let listEl = $state<HTMLElement | null>(null);
  let composerEl = $state<HTMLTextAreaElement | null>(null);
  let composerHostEl = $state<HTMLElement | null>(null);
  let composerHeight = $state(76);

  let isScrolledToBottom = $state(true);
  let isAtMaxBottom = $state(true);
  let farFromBottom = $state(false);
  let unreadCount = $state(0);
  let unreadBarShown = $state(false);
  // Where the view sat when the user marked the chat unread, so the mark can be
  // released the first time they move off (or onto) the bottom themselves.
  let bottomAtMark = $state<boolean | null>(null);
  let needInitialScroll = $state(false);
  let loadingOlder = $state(false);
  let loadingNewer = $state(false);
  let jumping = $state(false);
  let activePinId = $state<string | null>(null);
  let isDraggingFile = $state(false);
  interface MediaItem {
    src: string;
    path: string;
    kind: 'image' | 'video';
    filename: string;
  }
  // The viewer navigates within a single "batch" (the stack it was opened from,
  // or just the one image for a standalone bubble), not all chat media.
  let mediaViewer = $state<{ items: MediaItem[]; index: number } | null>(null);
  let pendingFiles = $state<
    {
      backendPath: string;
      filename: string;
      sizeBytes: number;
      payloadId?: string;
    }[]
  >([]);
  let sendingFile = $state(false);
  // Several shares can be staged into one composer, so every claimed payload is
  // tracked until the files are sent or dropped.
  let sharePayloadIds = $state<string[]>([]);
  let shareCaption = $state('');
  let unlistenDrop: (() => void) | null = null;
  let prevMessagesLength = $state(0);
  let prevLastId = $state<string | null>(null);
  let prevFirstId = $state<string | null>(null);
  let pendingScrollFrame = 0;
  let windowFocused = $state(true);
  let lastFocusedBeforeModal = $state<HTMLElement | null>(null);

  let swipeStart = { x: 0, y: 0, id: '' };
  let swipeOffset = $state<{ id: string; dx: number } | null>(null);

  let searchOpen = $state(false);
  let searchQuery = $state('');
  let matchIds = $state<string[]>([]);
  let matchPos = $state(0);

  let dismissedBanner = $state(false);
  let showShareBanner = $derived(browser && contact && !contact.profileShared && !dismissedBanner);

  let flushingQueue = $state(false);
  const pendingUploadCount = $derived(
    contactId ? messagesState.pendingUploadFor(contactId).length : 0
  );
  const inMailboxCount = $derived(contactId ? messagesState.inMailboxFor(contactId).length : 0);
  const delayedCount = $derived(contactId ? messagesState.delayedUnseenFor(contactId).length : 0);

  // Jumps to the earliest (topmost by sent-time) delayed message the user hasn't
  // seen yet, loading its page if it's outside the current window.
  async function jumpToEarliestDelayed() {
    if (!contactId) return;
    const ids = messagesState.delayedUnseenFor(contactId);
    if (ids.length === 0) return;
    const loaded = messagesState.forContact(contactId);
    const withTs = ids
      .map((id) => ({ id, ts: loaded.find((m) => m.id === id)?.timestamp ?? Infinity }))
      .sort((a, b) => a.ts - b.ts);
    const earliest = withTs[0].id;
    if (!scrollToMessage(earliest)) {
      jumping = true;
      const ok = await messagesState.loadAround(contactId, earliest);
      jumping = false;
      if (ok) {
        await tick();
        scrollToMessage(earliest);
      }
    }
    messagesState.markDelayedSeen(contactId, earliest);
  }

  // Clear a delayed marker once its message actually scrolls into view.
  $effect(() => {
    const cid = contactId;
    const ids = cid ? messagesState.delayedUnseenFor(cid) : [];
    if (!cid || ids.length === 0 || !listEl) return;
    const obs = new IntersectionObserver(
      (entries) => {
        for (const e of entries) {
          if (!e.isIntersecting) continue;
          const id = (e.target as HTMLElement).dataset.msgId;
          if (id) messagesState.markDelayedSeen(cid, id);
          obs.unobserve(e.target);
        }
      },
      { root: listEl, threshold: 0.5 }
    );
    for (const id of ids) {
      const el = listEl.querySelector<HTMLElement>(`[data-msg-id="${id}"]`);
      if (el) obs.observe(el);
    }
    return () => obs.disconnect();
  });
  const terminated = $derived(!!contactId && contactsState.isTerminated(contactId));
  const peerOnline = $derived.by(() => {
    if (!contactId) return false;
    return isOnlineStatus(contactsState.connectionStatus[contactId]);
  });
  // Reactions/edits/deletes ride the same offline queue but aren't messages, so
  // they only show up as "waiting to sync" markers. Both feed the flush timer.
  const queueActivity = $derived(
    contactId ? pendingUploadCount + messagesState.pendingSyncCountFor(contactId) : 0
  );
  const OFFLINE_FLUSH_DELAY = 5000;

  // Nudge the queue when the peer becomes reachable: the backend's own reconnect
  // flush is threshold-gated, so a short queue can sit untouched otherwise. This
  // doesn't clear "waiting to sync" markers; those clear on `offline_queue_drained`.
  $effect(() => {
    if (!peerOnline || !contactId) return;
    const cid = contactId;
    untrack(() => {
      autoFlushQueue(cid);
    });
  });

  async function handleShareQuickProfile() {
    if (!browser || !contactId) return;
    await shareBusy.run(async () => {
      try {
        await shareProfile(profileState.displayName, profileState.avatarBytes, contactId);
        const online = isOnlineStatus(contactsState.connectionStatus[contactId]);
        if (contact) contactsState.upsert({ ...contact, profileShared: true });
        notifications.push(
          online
            ? t('chat.conversation.successProfileShared')
            : t('chat.conversation.successProfileQueued', {
                name: contact?.displayName ?? '',
              }),
          'success'
        );
      } catch (e) {
        notifications.push(t('chat.conversation.errorProfileShare'), 'error');
        log.error(e);
      }
    });
  }

  function closeShareBanner() {
    dismissedBanner = true;
  }

  function openProfileModal() {
    lastFocusedBeforeModal = document.activeElement as HTMLElement | null;
    showProfile = true;
  }
  function closeProfileModal() {
    showProfile = false;
    tick().then(() => lastFocusedBeforeModal?.focus());
  }
  function openSecurityCodeModal() {
    lastFocusedBeforeModal = document.activeElement as HTMLElement | null;
    showSecurityCode = true;
  }
  function closeSecurityCodeModal() {
    showSecurityCode = false;
    tick().then(() => lastFocusedBeforeModal?.focus());
  }
  function openConnectionInfoModal() {
    lastFocusedBeforeModal = document.activeElement as HTMLElement | null;
    showConnectionInfo = true;
  }
  function closeConnectionInfoModal() {
    showConnectionInfo = false;
    tick().then(() => lastFocusedBeforeModal?.focus());
  }

  // Only user-driven scrolls dismiss the reaction picker; programmatic scrolls
  // (new-message auto-scroll, resize re-pin) must not, or the picker vanishes
  // the moment a message arrives.
  let programmaticScrollUntil = 0;
  function markProgrammaticScroll(ms = 300) {
    programmaticScrollUntil = Date.now() + ms;
  }

  function handleScroll() {
    if (showEmojiPicker && Date.now() >= programmaticScrollUntil) {
      showEmojiPicker = null;
      emojiPickerAnchor = null;
    }
    if (!listEl || pendingScrollFrame) return;
    pendingScrollFrame = window.requestAnimationFrame(() => {
      pendingScrollFrame = 0;
      if (!listEl) return;
      const { scrollTop, scrollHeight, clientHeight } = listEl;
      const distFromBottom = scrollHeight - scrollTop - clientHeight;
      isScrolledToBottom = Math.abs(distFromBottom) < 60;
      isAtMaxBottom = distFromBottom <= 2;
      farFromBottom = distFromBottom > 120;
      if (contactId) scrollMemory.set(contactId, scrollTop);
      if (isAtMaxBottom && unreadCount > 0) unreadCount = 0;
      scheduleRead();
      updateUnreadBar();
      if (scrollTop < 300) void maybeLoadOlder();
      if (distFromBottom < 300 && !messagesState.isNewestReached(contactId)) void maybeLoadNewer();
      updateActivePin();
    });
  }

  // Landing at the bottom isn't reading: an initial scroll, a jump or a send
  // all get you there without a message having been looked at. The dwell has
  // to elapse with the view still parked there and the window still focused.
  const READ_DWELL_MS = 2500;
  let readTimer: ReturnType<typeof setTimeout> | null = null;

  function cancelScheduledRead() {
    if (!readTimer) return;
    clearTimeout(readTimer);
    readTimer = null;
  }

  function scheduleRead() {
    if (!contactId || !isAtMaxBottom || !windowFocused || pendingUnread === 0) {
      cancelScheduledRead();
      return;
    }
    if (readTimer) return;
    const id = contactId;
    readTimer = setTimeout(() => {
      readTimer = null;
      if (id !== contactId || !isAtMaxBottom || !windowFocused) return;
      messagesState.markRead(id);
    }, READ_DWELL_MS);
  }

  // The pin bar shows whichever pinned message you're currently reading: the
  // last one scrolled past (nearest above the anchor line), else the next one
  // coming up. Measured from already-rendered nodes: cheap, no extra listener.
  const PIN_ANCHOR = 56;
  function updateActivePin() {
    if (!listEl || !contactId) return;
    const pins = messagesState.pinnedFor(contactId);
    if (pins.length === 0) {
      if (activePinId !== null) activePinId = null;
      return;
    }
    const listTop = listEl.getBoundingClientRect().top;
    let above: string | null = null;
    let aboveTop = -Infinity;
    let below: string | null = null;
    let belowTop = Infinity;
    for (const m of pins) {
      const el = listEl.querySelector<HTMLElement>(`[data-msg-id="${m.id}"]`);
      if (!el) continue;
      const top = el.getBoundingClientRect().top - listTop;
      if (top <= PIN_ANCHOR) {
        if (top > aboveTop) {
          aboveTop = top;
          above = m.id;
        }
      } else if (top < belowTop) {
        belowTop = top;
        below = m.id;
      }
    }
    const next = above ?? below;
    if (next) {
      if (next !== activePinId) activePinId = next;
    } else if (activePinId && !pins.some((p) => p.id === activePinId)) {
      activePinId = null;
    }
  }

  // Jump that lands the pinned message just under the bar (not centered) so the
  // scroll-driven active pin resolves to it, keeping the bar in sync.
  // The separator is what tells you where the unread run starts, so once it
  // scrolls off the top the bar stands in for it.
  function updateUnreadBar() {
    if (!listEl || unreadRunCount === 0 || !firstUnreadId) {
      unreadBarShown = false;
      return;
    }
    const sep = listEl.querySelector<HTMLElement>('.unread-separator');
    if (!sep) {
      unreadBarShown = true;
      return;
    }
    unreadBarShown = sep.getBoundingClientRect().bottom < listEl.getBoundingClientRect().top;
  }

  async function jumpToUnread() {
    if (!firstUnreadId) return;
    let sep = listEl?.querySelector<HTMLElement>('.unread-separator');
    if (!sep && contactId) {
      jumping = true;
      const ok = await messagesState.loadAround(contactId, firstUnreadId);
      await tick();
      jumping = false;
      if (!ok) return;
      sep = listEl?.querySelector<HTMLElement>('.unread-separator');
    }
    if (!sep || !listEl) return;
    markProgrammaticScroll(600);
    const delta = sep.getBoundingClientRect().top - listEl.getBoundingClientRect().top - 8;
    listEl.scrollTo({ top: listEl.scrollTop + delta, behavior: 'smooth' });
    isScrolledToBottom = false;
    isAtMaxBottom = false;
    farFromBottom = true;
  }

  async function jumpToPinned(id: string) {
    let el = listEl?.querySelector<HTMLElement>(`[data-msg-id="${id}"]`);
    if (!el && contactId) {
      jumping = true;
      const ok = await messagesState.loadAround(contactId, id);
      await tick();
      jumping = false;
      if (!ok) {
        notifications.push(t('chat.conversation.messageNotFound'), 'info');
        return;
      }
      el = listEl?.querySelector<HTMLElement>(`[data-msg-id="${id}"]`);
    }
    if (!el || !listEl) return;
    const delta = el.getBoundingClientRect().top - listEl.getBoundingClientRect().top - 8;
    listEl.scrollTo({ top: listEl.scrollTop + delta, behavior: 'smooth' });
    isScrolledToBottom = false;
    isAtMaxBottom = false;
    farFromBottom = true;
    el.classList.add('flash');
    setTimeout(() => el?.classList.remove('flash'), 1200);
  }

  $effect(() => {
    void messagesState.pinnedFor(contactId);
    void messagesState.forContact(contactId);
    if (!listEl) return;
    const raf = requestAnimationFrame(updateActivePin);
    return () => cancelAnimationFrame(raf);
  });

  $effect(() => {
    void firstUnreadId;
    void unreadRunCount;
    void messages.length;
    if (!listEl) return;
    const raf = requestAnimationFrame(updateUnreadBar);
    return () => cancelAnimationFrame(raf);
  });

  async function maybeLoadOlder() {
    if (loadingOlder || !listEl || messagesState.isOldestReached(contactId)) return;
    loadingOlder = true;
    const el = listEl;
    const prevHeight = el.scrollHeight;
    const prevTop = el.scrollTop;
    const added = await messagesState.loadOlder(contactId);
    if (added > 0) {
      await tick();
      el.scrollTop = prevTop + (el.scrollHeight - prevHeight);
    }
    loadingOlder = false;
  }

  async function maybeLoadNewer() {
    if (loadingNewer || !listEl || messagesState.isNewestReached(contactId)) return;
    loadingNewer = true;
    const el = listEl;
    const prevTop = el.scrollTop;
    const added = await messagesState.loadNewer(contactId);
    if (added > 0) {
      await tick();
      el.scrollTop = prevTop;
    }
    loadingNewer = false;
  }

  const emojiPickerPos = $derived(
    emojiPickerAnchor
      ? emojiPickerPosition(emojiPickerAnchor, window.innerWidth, window.innerHeight)
      : null
  );

  const PICKER_CENTER_STYLE = 'top:50%;left:50%;transform:translate(-50%,-50%);';

  // Only one of top/bottom is set, so the picker keeps the edge facing the
  // message fixed while search results change its height. The geometry is
  // computed against the raw viewport, so the insets are clamped back in here.
  function emojiPickerLayerStyle(pos: EmojiPickerPos): string {
    const vertical =
      pos.bottom !== null
        ? `bottom:max(${pos.bottom}px, calc(var(--safe-bottom) + 8px));`
        : `top:max(${pos.top}px, calc(var(--safe-top) + 8px));`;
    const minLeft = 'calc(var(--safe-left) + 8px)';
    const maxLeft = `max(${minLeft}, calc(100% - var(--safe-right) - ${PICKER_W + 8}px))`;
    return `${vertical}left:clamp(${minLeft}, ${pos.left}px, ${maxLeft});`;
  }

  async function handlePickerSelect(emoji: string, keepOpen: boolean) {
    if (!showEmojiPicker) return;
    const msg = messageIndex.get(showEmojiPicker);
    if (msg) await toggleReaction(msg, emoji);
    if (keepOpen) return;
    showEmojiPicker = null;
    emojiPickerAnchor = null;
  }
  function closePicker() {
    showEmojiPicker = null;
    emojiPickerAnchor = null;
  }

  function scrollToBottom(behavior: ScrollBehavior = 'auto') {
    if (!listEl) return;
    markProgrammaticScroll();
    if (behavior === 'auto') listEl.scrollTop = listEl.scrollHeight;
    else listEl.scrollTo({ top: listEl.scrollHeight, behavior });
    unreadCount = 0;
    isScrolledToBottom = true;
    isAtMaxBottom = true;
    farFromBottom = false;
    scheduleRead();
  }

  // Pins to the bottom across a few frames: late layout (images, avatars,
  // wrapped text) can grow the list after a single scroll would have landed.
  function pinBottomFrames(frames: number) {
    const id = contactId;
    let f = 0;
    const pin = () => {
      if (!listEl || id !== contactId) return;
      markProgrammaticScroll();
      listEl.scrollTop = listEl.scrollHeight;
      if (++f < frames) requestAnimationFrame(pin);
    };
    pin();
  }

  function pinInitialBottom() {
    isScrolledToBottom = true;
    isAtMaxBottom = true;
    farFromBottom = false;
    pinBottomFrames(5);
  }

  function updateScrollFlags() {
    if (!listEl) return;
    const { scrollTop, scrollHeight, clientHeight } = listEl;
    const dist = scrollHeight - scrollTop - clientHeight;
    isScrolledToBottom = Math.abs(dist) < 60;
    isAtMaxBottom = dist <= 2;
    farFromBottom = dist > 120;
  }

  // Returns to the live tail: if we're in a jumped window, reload the newest
  // page first, then scroll to the bottom.
  async function jumpToLatest() {
    if (!messagesState.isNewestReached(contactId)) {
      jumping = true;
      await messagesState.loadFor(contactId);
      await tick();
      jumping = false;
    }
    scrollToBottom('smooth');
  }

  const visibleMessages = $derived(sortByOfflineTier(messages, peerOnline));

  // Turns a message's downloaded media into a viewer item, or null if its file
  // isn't on disk yet (nothing to show).
  function mediaItemFor(m: MessageResponse): MediaItem | null {
    const fd = m.fileDetails;
    if (!fd?.autodownloadPath) return null;
    const k = mediaKindFromFilename(fd.filename);
    if (k !== 'image' && k !== 'video') return null;
    return {
      src: mediaUrl(fd.autodownloadPath, messagesState.mediaVersionFor(m.contactId, m.id)),
      path: fd.autodownloadPath,
      kind: k,
      filename: fd.filename,
    };
  }
  function mediaPrev() {
    if (mediaViewer && mediaViewer.index > 0) {
      mediaViewer = { ...mediaViewer, index: mediaViewer.index - 1 };
    }
  }
  function mediaNext() {
    if (mediaViewer && mediaViewer.index < mediaViewer.items.length - 1) {
      mediaViewer = { ...mediaViewer, index: mediaViewer.index + 1 };
    }
  }

  const messageGroups = $derived(buildMessageGroups(visibleMessages, peerOnline, firstUnreadId));

  const transferPercentFor = (transferId: string): number =>
    transferPercent(messagesState.transferProgressFor(transferId));
  const isTransferDoneFor = (transferId: string): boolean =>
    isTransferDone(messagesState.transferProgressFor(transferId));

  function resizeComposer() {
    if (!composerEl) return;
    composerEl.style.height = 'auto';
    composerEl.style.height = `${Math.min(composerEl.scrollHeight, 180)}px`;
  }

  let lastTypingSentAt = 0;
  function maybeSendTyping() {
    if (!contactId) return;
    if (editingMessageId) {
      lastTypingSentAt = 0;
      return;
    }
    if (!settingsState.typingIndicators) {
      lastTypingSentAt = 0;
      return;
    }
    if (!inputText.trim()) {
      lastTypingSentAt = 0;
      return;
    }
    const now = Date.now();
    if (now - lastTypingSentAt < 3500) return;
    lastTypingSentAt = now;
    void sendTypingIndicator(contactId, replyingToMessageId).catch((e) => {
      log.error('send_typing_indicator failed', e);
    });
  }

  function onComposerInput() {
    resizeComposer();
    maybeSendTyping();
  }

  const messageIndex = $derived.by(() => {
    const byId = new Map<string, MessageResponse>();
    for (const msg of messages) byId.set(msg.id, msg);
    return byId;
  });

  const replyingToPreview = $derived(
    replyingToMessageId
      ? getMessagePreview(messageIndex.get(replyingToMessageId)?.content ?? '')
      : ''
  );
  const editingPreview = $derived(
    editingMessageId ? getMessagePreview(messageIndex.get(editingMessageId)?.content ?? '') : ''
  );

  function startReply(msg: MessageResponse) {
    replyingToMessageId = msg.id;
    editingMessageId = null;
    actionSheetMsgId = null;
    tick().then(() => composerEl?.focus());
  }
  function startEdit(msg: MessageResponse) {
    editingMessageId = msg.id;
    replyingToMessageId = null;
    inputText = msg.content;
    actionSheetMsgId = null;
    tick().then(() => composerEl?.focus());
  }
  function editLastMessage() {
    if (!contactId) return;
    const list = messagesState.forContact(contactId);
    for (let i = list.length - 1; i >= 0; i--) {
      const m = list[i];

      if (m.direction === 'sent' && !m.fileDetails && m.content && isMessageActionable(m.status)) {
        startEdit(m);
        return;
      }
    }
  }
  function cancelEdit() {
    editingMessageId = null;
    inputText = '';
  }
  function cancelReply() {
    replyingToMessageId = null;
  }

  async function handleDelete(msg: MessageResponse) {
    actionSheetMsgId = null;
    const cid = contactId;
    const mid = msg.id;
    messagesState.setPendingDelete(mid, true);
    try {
      const queuedOffline = await deleteMessage(cid, mid);
      messagesState.setPendingDelete(mid, false);
      if (queuedOffline) {
        messagesState.markPendingSync(cid, mid, true);
      } else {
        messagesState.markDeleted(mid, cid);
      }
      notifications.push(t('chat.conversation.successDeleted'), 'info');
    } catch (e) {
      log.error('Delete failed', e);
      messagesState.setPendingDelete(mid, false);
      notifications.push(t('chat.conversation.errorDeleteMessage'), 'error');
    }
  }

  async function toggleReaction(msg: MessageResponse, emoji: string) {
    void haptics.select();
    try {
      const reactions = messagesState.reactionsFor(msg.id, contactId);
      const isReacted = reactions
        .find((r) => r.emoji === emoji)
        ?.userIds.includes(profileState.userId || '');
      let queuedOffline: boolean;
      if (isReacted) {
        queuedOffline = await removeReaction(contactId, msg.id, emoji);
        messagesState.removeReaction(msg.id, contactId, emoji, profileState.userId || '');
      } else {
        queuedOffline = await addReaction(contactId, msg.id, emoji);
        messagesState.addReaction(msg.id, contactId, emoji, profileState.userId || '');
      }
      if (queuedOffline) messagesState.markPendingSync(contactId, msg.id);
    } catch (e) {
      log.error('React failed', e);
      notifications.push(t('chat.conversation.errorReact'), 'error');
    }
    showEmojiPicker = null;
    actionSheetMsgId = null;
  }

  async function copyMessageText(msg: MessageResponse) {
    actionSheetMsgId = null;
    try {
      await navigator.clipboard.writeText(msg.content);
      messageFlash.trigger(msg.id);
    } catch {
      notifications.push(t('chat.conversation.errorCopy'), 'error');
    }
  }

  function scrollToMessage(id: string): boolean {
    const el = listEl?.querySelector<HTMLElement>(`[data-msg-id="${id}"]`);
    if (!el) return false;
    el.scrollIntoView({ behavior: 'smooth', block: 'center' });
    isScrolledToBottom = false;
    isAtMaxBottom = false;
    farFromBottom = true;
    el.classList.add('flash');
    setTimeout(() => el.classList.remove('flash'), 1200);
    return true;
  }

  async function handleReplyRefClick(replyToId: string) {
    if (scrollToMessage(replyToId)) return;
    const id = contactId;
    if (id) {
      jumping = true;
      const ok = await messagesState.loadAround(id, replyToId);
      await tick();
      jumping = false;
      if (ok && scrollToMessage(replyToId)) return;
    }
    notifications.push(t('chat.conversation.messageNotFound'), 'info');
  }

  function searchNext() {
    if (!matchIds.length) return;
    matchPos = (matchPos + 1) % matchIds.length;
    handleReplyRefClick(matchIds[matchPos]);
  }
  function searchPrev() {
    if (!matchIds.length) return;
    matchPos = (matchPos - 1 + matchIds.length) % matchIds.length;
    handleReplyRefClick(matchIds[matchPos]);
  }
  function togglePin(msg: MessageResponse) {
    const next = !msg.pinned;
    messagesState.setPinnedOptimistic(msg.contactId, msg.id, next);
    void pinMessage(msg.contactId, msg.id, next).catch((e) => {
      log.error('pin failed', e);
      messagesState.setPinnedOptimistic(msg.contactId, msg.id, !next);
    });
  }

  function markUnreadFrom(msg: MessageResponse) {
    messagesState.markUnreadFrom(msg.contactId, msg.id);
    bottomAtMark = isAtMaxBottom;
    void haptics.impact('light');
  }

  // Forced: reading by hand has to override a conversation the user themselves
  // put back to unread. The separator goes with it, or the bar would linger.
  function markCurrentRead() {
    if (!contactId) return;
    messagesState.markRead(contactId, true);
    messagesState.clearFirstUnread(contactId);
    bottomAtMark = null;
    unreadBarShown = false;
    void haptics.impact('light');
  }

  function openSearch() {
    searchOpen = true;
  }
  function closeSearch() {
    searchOpen = false;
    searchQuery = '';
    matchIds = [];
    matchPos = 0;
  }

  async function handleDeleteLocal(msg: MessageResponse, silent = false) {
    messagesState.removeLocally(msg.id, msg.contactId);
    void haptics.impact('light');
    try {
      await deleteLocalMessage(msg.contactId, msg.id);
      if (!silent) notifications.push(t('chat.conversation.successDeleted'), 'success');
    } catch (e) {
      log.error('delete_local_message backend error:', e);
      if (!silent) notifications.push(t('chat.conversation.successDeleted'), 'success');
    }
  }

  // Re-runs the full send lifecycle for a failed message. The backend reuses the
  // same MessageId (kept in place, status flips back to sending), so a duplicate
  // arrival collapses to one message: no delete-and-resend, no duplicate.
  async function handleRetry(msg: MessageResponse) {
    void haptics.impact('medium');
    await messagesState.retryMessage(msg.contactId, msg.id);
  }

  onMount(() => {
    isCoarsePointer = window.matchMedia('(pointer: coarse)').matches;
    windowFocused = document.hasFocus() && document.visibilityState === 'visible';

    const onFocus = () => {
      windowFocused = document.visibilityState === 'visible';
    };
    const onBlur = () => {
      windowFocused = false;
    };
    const onVisibility = () => {
      windowFocused = document.visibilityState === 'visible' && document.hasFocus();
      if (document.visibilityState === 'hidden') autoFlushQueue(contactId);
    };
    window.addEventListener('focus', onFocus);
    window.addEventListener('blur', onBlur);
    document.addEventListener('visibilitychange', onVisibility);

    const onGlobalKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && e.shiftKey) {
        e.preventDefault();
        markCurrentRead();
        return;
      }
      if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
        e.preventDefault();
        if (searchOpen) closeSearch();
        else openSearch();
        return;
      }
      if (e.key === 'Escape' && searchOpen) {
        closeSearch();
        return;
      }
      if (
        e.key === 'Escape' &&
        !replyingToMessageId &&
        !editingMessageId &&
        !showSecurityCode &&
        !showProfile &&
        !mediaViewer &&
        !showEmojiPicker &&
        !actionSheetMsgId &&
        !forwardContent &&
        !selectTextMsgId
      ) {
        e.preventDefault();
        void jumpToLatest();
        return;
      }
      {
        const act = document.activeElement;
        const typing =
          act instanceof HTMLInputElement ||
          act instanceof HTMLTextAreaElement ||
          (act instanceof HTMLElement && act.isContentEditable);
        if (!typing && listEl) {
          if (e.key === 'End' || ((e.metaKey || e.ctrlKey) && e.key === 'ArrowDown')) {
            e.preventDefault();
            scrollToBottom('smooth');
            return;
          }
          if (e.key === 'PageDown') {
            e.preventDefault();
            listEl.scrollBy({
              top: listEl.clientHeight * 0.9,
              behavior: 'smooth',
            });
            return;
          }
          if (e.key === 'PageUp') {
            e.preventDefault();
            listEl.scrollBy({
              top: -listEl.clientHeight * 0.9,
              behavior: 'smooth',
            });
            return;
          }
        }
      }
      if (!composerEl) return;
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key.length !== 1) return;
      const ae = document.activeElement;
      if (ae === composerEl) return;
      if (ae instanceof HTMLInputElement || ae instanceof HTMLTextAreaElement) return;
      if (ae instanceof HTMLElement && ae.isContentEditable) return;
      composerEl.focus();
    };
    window.addEventListener('keydown', onGlobalKey);

    void (async () => {
      try {
        const { getCurrentWebview } = await import('@tauri-apps/api/webview');
        unlistenDrop = await getCurrentWebview().onDragDropEvent((event) => {
          const p = event.payload;
          if (p.type === 'enter' || p.type === 'over') {
            isDraggingFile = !contactDropTargetAt(p.position);
          } else if (p.type === 'leave') {
            isDraggingFile = false;
          } else if (p.type === 'drop') {
            isDraggingFile = false;
            // A drop on a sidebar contact row belongs to the layout router.
            if (contactDropTargetAt(p.position)) return;
            const paths = (p as { paths?: string[] }).paths ?? [];
            void handleDroppedPaths(paths);
          }
        });
      } catch (e) {
        log.warn('Drag/drop not available:', e);
      }
    })();

    let contactMissingTimer: ReturnType<typeof setTimeout> | null = null;
    if (!contact && !contactsState.loading) {
      contactMissingTimer = setTimeout(() => {
        if (contactId && !contactsState.getById(contactId)) {
          notifications.push(t('chat.conversation.errorContactNotFound'), 'error');
          goto('/chat', { replaceState: true });
        }
      }, 100);
    }

    return () => {
      window.removeEventListener('focus', onFocus);
      window.removeEventListener('blur', onBlur);
      window.removeEventListener('keydown', onGlobalKey);
      document.removeEventListener('visibilitychange', onVisibility);
      if (contactMissingTimer) clearTimeout(contactMissingTimer);
      if (pendingScrollFrame) window.cancelAnimationFrame(pendingScrollFrame);
      for (const t of completedFileTimers.values()) clearTimeout(t);
      completedFileTimers.clear();
      unlistenDrop?.();
      unlistenDrop = null;
    };
  });

  $effect(() => {
    const id = contactId;
    if (!id) return;
    untrack(() => messagesState.enterChat(id));
    bottomAtMark = null;
    const jump = uiState.pendingMessageJump;
    const wantJump = !!jump && jump.contactId === id;
    if (wantJump) {
      // Suppress the initial scroll-to-bottom; we scroll to the target instead.
      needInitialScroll = false;
      jumping = true;
    } else {
      needInitialScroll = true;
      jumping = false;
    }
    // Read before the load: reaching the bottom while messages stream in can
    // mark them read, and the separator has to reflect the count on arrival.
    const unreadAtOpen = untrack(() => messagesState.unreadFor(id));
    void messagesState.loadFor(id).then(async () => {
      if (id !== contactId) return;
      if (wantJump && jump) {
        uiState.pendingMessageJump = null;
        await tick();
        if (!scrollToMessage(jump.messageId)) {
          const ok = await messagesState.loadAround(id, jump.messageId);
          await tick();
          if (!ok || !scrollToMessage(jump.messageId))
            notifications.push(t('chat.conversation.messageNotFound'), 'info');
        }
        jumping = false;
        return;
      }
      if (unreadAtOpen > 0 && !messagesState.firstUnreadFor(id)) {
        const list = messagesState.forContact(id);
        const idx = Math.max(0, list.length - unreadAtOpen);
        const first = list.slice(idx).find((m) => m.direction === 'received');
        if (first) messagesState.setFirstUnread(id, first.id);
      }
    });
    return () => {
      messagesState.leaveChat(id);
    };
  });

  $effect(() => {
    const id = contactId;
    if (!id) return;
    inputText = untrack(() => draftsState.get(id));
    replyingToMessageId = null;
    editingMessageId = null;
    return () => {
      if (editingMessageId) return;
      draftsState.set(id, inputText);
    };
  });

  $effect(() => {
    const id = contactId;
    const text = inputText;
    if (!id) return;
    // While editing, inputText holds the edit buffer, not a draft.
    if (untrack(() => editingMessageId)) return;
    draftsState.set(id, text);
  });

  // Picks up a payload the share sheet handed over, once this chat is the one
  // the user chose. Files were already staged on disk by the native side, so
  // they go straight into the confirm modal.
  $effect(() => {
    const id = contactId;
    const head = shareIntentState.head;
    if (!id || !head) return;
    const payload = shareIntentState.claim(id);
    if (payload) untrack(() => applySharePayload(payload));
  });

  function applySharePayload(payload: SharePayload) {
    const text = payload.text?.trim() ?? '';

    if (!payload.files.length) {
      if (text) inputText = inputText ? `${inputText}\n${text}` : text;
      void shareIntentState.release(payload.id);
      return;
    }

    const known = new Set(pendingFiles.map((f) => f.backendPath));
    const staged = payload.files
      .filter((f) => !known.has(f.path))
      .map((f) => ({
        backendPath: f.path,
        filename: f.filename,
        sizeBytes: f.sizeBytes,
        payloadId: payload.id,
      }));

    if (!staged.length) {
      void shareIntentState.release(payload.id);
      return;
    }

    sharePayloadIds = [...sharePayloadIds, payload.id];
    if (text) shareCaption = shareCaption ? `${shareCaption}\n${text}` : text;
    pendingFiles = [...pendingFiles, ...staged];
  }

  // Releasing a payload deletes its staged files, so only payloads with no file
  // left in the composer are freed: a share claimed mid-send keeps its files.
  function releaseSharedFiles() {
    const held = new Set(
      pendingFiles.map((f) => f.payloadId).filter((id): id is string => id !== undefined)
    );
    const released = sharePayloadIds.filter((id) => !held.has(id));
    sharePayloadIds = sharePayloadIds.filter((id) => held.has(id));
    if (!sharePayloadIds.length) shareCaption = '';
    for (const id of released) void shareIntentState.release(id);
  }

  let searchSeq = 0;
  $effect(() => {
    const q = searchQuery.trim();
    const id = contactId;
    if (!q || !id) {
      matchIds = [];
      matchPos = 0;
      return;
    }
    const seq = ++searchSeq;
    const handle = setTimeout(async () => {
      try {
        const results = await searchMessages(id, q, 200);
        if (seq !== searchSeq || id !== contactId) return;
        matchIds = results.map((m) => m.id);
        matchPos = 0;
        if (matchIds.length) handleReplyRefClick(matchIds[0]);
      } catch (e) {
        log.error('Message search failed:', e);
      }
    }, 200);
    return () => clearTimeout(handle);
  });

  $effect(() => {
    if (!listEl) return;
    const el = listEl;
    const handleClick = (e: Event) => {
      void handleMarkdownClick(e as MouseEvent);
    };
    el.addEventListener('click', handleClick);
    return () => {
      el.removeEventListener('click', handleClick);
    };
  });

  $effect(() => {
    if (!composerHostEl) return;
    const el = composerHostEl;
    const ro = new ResizeObserver(() => {
      composerHeight = el.offsetHeight;
    });
    // border-box: the keyboard inset lands on this element's padding, which a
    // content-box observation would not report.
    ro.observe(el, { box: 'border-box' });
    composerHeight = el.offsetHeight;
    return () => ro.disconnect();
  });

  // Re-pin to bottom when content grows (image loads, file progress) OR when
  // the list itself shrinks (mobile keyboard opening steals viewport height),
  // as long as the user was already at the bottom.
  $effect(() => {
    if (!listEl) return;
    const el = listEl;
    const ro = new ResizeObserver(() => {
      if (isScrolledToBottom) {
        markProgrammaticScroll();
        el.scrollTop = el.scrollHeight;
      }
    });
    ro.observe(el);
    for (const child of Array.from(el.children)) {
      ro.observe(child);
    }
    const mo = new MutationObserver((mutations) => {
      for (const mut of mutations) {
        for (const node of mut.addedNodes) {
          if (node instanceof Element) ro.observe(node);
        }
      }
    });
    mo.observe(el, { childList: true });
    return () => {
      ro.disconnect();
      mo.disconnect();
    };
  });

  $effect(() => {
    const len = messages.length;
    const lastId = len > 0 ? messages[len - 1].id : null;
    const firstId = len > 0 ? messages[0].id : null;
    if (needInitialScroll && len > 0 && listEl) {
      needInitialScroll = false;
      prevMessagesLength = len;
      prevLastId = lastId;
      prevFirstId = firstId;
      // Position synchronously (DOM is already updated when this effect runs),
      // so the chat is at its spot on first paint: no visible top→bottom scroll.
      const savedTop = scrollMemory.get(contactId);
      const sep = firstUnreadId ? listEl.querySelector<HTMLElement>('.unread-separator') : null;
      if (sep && unreadRunCount > UNREAD_TAIL_LIMIT) {
        sep.scrollIntoView({ block: 'center' });
        isScrolledToBottom = false;
        isAtMaxBottom = false;
        farFromBottom = true;
      } else if (savedTop != null && unreadRunCount === 0) {
        listEl.scrollTop = savedTop;
        updateScrollFlags();
      } else {
        pinInitialBottom();
      }
      return;
    }
    // Pagination / jumps reshape the list without a genuine new message.
    if (loadingOlder || loadingNewer || jumping) {
      prevMessagesLength = len;
      prevLastId = lastId;
      prevFirstId = firstId;
      return;
    }

    const prepended = prevMessagesLength > 0 && firstId !== prevFirstId;
    if (len > prevMessagesLength && !prepended) {
      const diff = len - prevMessagesLength;
      const tailChanged = lastId !== prevLastId;
      tick().then(() => {
        const lastMsg = messages[len - 1];
        // Pinning appends a pin line with direction 'sent', but it is not a
        // message the user just wrote. While reading history it must not pull
        // the view down, nor count as unread.
        if (lastMsg?.direction === 'sent' && lastMsg.pinDetails && !isScrolledToBottom) return;
        if ((tailChanged && lastMsg?.direction === 'sent') || isScrolledToBottom) {
          const behavior: ScrollBehavior = isScrolledToBottom ? 'auto' : 'smooth';
          scrollToBottom(behavior);

          if (behavior === 'auto') {
            requestAnimationFrame(() => pinBottomFrames(4));
          }
        } else {
          unreadCount += diff;
        }
      });
    }
    prevMessagesLength = len;
    prevLastId = lastId;
    prevFirstId = firstId;
  });

  // Suppression exists so the auto-read can't undo the click on the spot. Once
  // the user scrolls off the bottom, or reaches it from elsewhere, that is a
  // deliberate move and reading takes over again.
  $effect(() => {
    if (bottomAtMark === null || !contactId || isAtMaxBottom === bottomAtMark) return;
    bottomAtMark = null;
    messagesState.clearMarkedUnread(contactId);
  });

  $effect(() => {
    void contactId;
    void isAtMaxBottom;
    void windowFocused;
    // A message arriving while you sit at the bottom has to start its own dwell.
    void pendingUnread;
    scheduleRead();
    return cancelScheduledRead;
  });

  $effect(() => {
    for (const [msgId, st] of Object.entries(fileOfferActionState)) {
      if (st !== 'accepted') continue;
      if (!isTransferDoneFor(msgId)) continue;
      if (completedFileTimers.has(msgId)) continue;
      const timer = setTimeout(() => {
        fileOfferActionState[msgId] = 'idle';
        completedFileTimers.delete(msgId);
      }, 8000);
      completedFileTimers.set(msgId, timer);
    }
  });

  $effect(() => {
    const id = contactId;
    const unreadId = firstUnreadId;
    const _len = messages.length;
    if (!id || !unreadId) return;
    if (!windowFocused || !isAtMaxBottom) return;
    const timer = setTimeout(() => {
      messagesState.clearFirstUnread(id);
    }, 2000);
    return () => clearTimeout(timer);
  });

  $effect(() => {
    inputText;
    tick().then(() => {
      resizeComposer();
      if (isScrolledToBottom && listEl) {
        markProgrammaticScroll();
        listEl.scrollTop = listEl.scrollHeight;
      }
    });
  });

  const MAX_MESSAGE_LENGTH = 10000;

  async function handleSend() {
    const text = inputText.trim();
    if (!text) return;
    if (text.length > MAX_MESSAGE_LENGTH) {
      if (await offerTextAsFile(text)) inputText = '';
      return;
    }

    if (editingMessageId) {
      if (!contactId) return;
      sending = true;
      try {
        const queuedOffline = await editMessage(contactId, editingMessageId, text);
        messagesState.updateContent(editingMessageId, contactId, text);
        if (queuedOffline) messagesState.markPendingSync(contactId, editingMessageId);
        inputText = '';
        editingMessageId = null;
      } catch (e) {
        notifications.push(t('chat.conversation.errorEditMessage'), 'error');
        log.error('Edit failed:', e);
      } finally {
        sending = false;
      }
      return;
    }

    if (!contactId) return;
    messagesState.markRead(contactId, true);
    const replyTo = replyingToMessageId;
    inputText = '';
    draftsState.clear(contactId);
    lastTypingSentAt = 0;
    cancelReply();

    // Sending returns to the live tail so the new message lands in view.
    // Reloading the newest page swaps the whole list; pin to the bottom
    // synchronously (before paint) so it never flashes at the top first.
    if (!messagesState.isNewestReached(contactId)) {
      jumping = true;
      await messagesState.loadFor(contactId);
      await tick();
      if (listEl) {
        listEl.scrollTop = listEl.scrollHeight;
        updateScrollFlags();
      }
      jumping = false;
    }

    const cid = contactId;
    const pendingId = crypto.randomUUID().replace(/-/g, '');
    messagesState.appendOptimistic({
      id: pendingId,
      contactId: cid,
      direction: 'sent',
      content: text,
      status: 'sending',
      timestamp: Date.now(),
      receivedTimestamp: Date.now(),
      replyTo,
    });

    void haptics.impact('medium');

    void sendText(cid, text, replyTo)
      .then((realId) => messagesState.replaceId(pendingId, cid, realId))
      .catch((e) => {
        messagesState.updateStatusIfSending(pendingId, cid, 'queued');
        void haptics.notify('warning');
        log.error('Send failed, queued for offline:', e);
      });
  }

  async function stageFilesForSend(files: { backendPath: string; filename: string }[]) {
    if (!files.length) return;
    const staged = await Promise.all(
      files.map(async ({ backendPath, filename }) => {
        let sizeBytes = 0;
        try {
          const info = await stat(backendPath);
          sizeBytes = Number(info.size ?? 0);
        } catch {
          sizeBytes = 0;
        }
        return { backendPath, filename, sizeBytes };
      })
    );
    const known = new Set(pendingFiles.map((f) => f.backendPath));
    pendingFiles = [...pendingFiles, ...staged.filter((f) => !known.has(f.backendPath))];
  }

  async function handlePasteImage({ bytes, ext }: { bytes: Uint8Array; ext: string }) {
    try {
      await stageFilesForSend([await prepareOfferFromBytes(bytes, `pasted-image.${ext}`)]);
    } catch (e) {
      notifyError(e, 'chat.conversation.errorPasteImage');
    }
  }

  // Text past the limit can't be sent as a message: ask, then stage it as a
  // .txt attachment. Returns whether the file was staged.
  async function offerTextAsFile(text: string): Promise<boolean> {
    const ok = await confirmDialog({
      title: t('chat.conversation.tooLongTitle'),
      message: t('chat.conversation.tooLongMessage', {
        length: text.length,
        max: MAX_MESSAGE_LENGTH,
      }),
      confirmLabel: t('chat.conversation.tooLongConfirm'),
    });
    if (!ok) return false;
    try {
      const bytes = new TextEncoder().encode(text);
      await stageFilesForSend([await prepareOfferFromBytes(bytes, 'message.txt')]);
      return true;
    } catch (e) {
      notifyError(e, 'chat.conversation.errorPasteText');
      return false;
    }
  }

  let showAttachSheet = $state(false);

  function openAttach() {
    if (isMobile) {
      showAttachSheet = true;
    } else {
      void handleSendFile();
    }
  }

  async function handleSendFile(pickerMode?: PickerMode) {
    try {
      const prepared = await pickFilesForSend(pickerMode);
      if (!prepared.length) return;
      await stageFilesForSend(prepared);
    } catch (e) {
      notifyError(e, 'chat.conversation.errorOpenFile');
    }
  }

  async function handlePickedFiles(files: File[]) {
    try {
      const prepared = await Promise.all(
        files.map((f, i) =>
          prepareOfferFromFile(f, `capture-${i + 1}.${f.type.split('/')[1] ?? 'bin'}`)
        )
      );
      await stageFilesForSend(prepared);
    } catch (e) {
      notifyError(e, 'chat.conversation.errorOpenFile');
    }
  }

  async function handleDroppedPaths(paths: string[]) {
    if (!paths?.length || !contactId) return;
    try {
      const prepared = await Promise.all(paths.map((p) => prepareOfferSourcePath(p)));
      await stageFilesForSend(prepared);
    } catch (e) {
      notifyError(e, 'chat.conversation.errorPrepareFile');
    }
  }

  // Caption rides along with a file offer but is sent as its own text message.
  function sendCaption(cid: string, text: string) {
    const pendingId = crypto.randomUUID().replace(/-/g, '');
    messagesState.appendOptimistic({
      id: pendingId,
      contactId: cid,
      direction: 'sent',
      content: text,
      status: 'sending',
      timestamp: Date.now(),
      receivedTimestamp: Date.now(),
      replyTo: null,
    });
    void sendText(cid, text, null)
      .then((realId) => messagesState.replaceId(pendingId, cid, realId))
      .catch((e) => {
        messagesState.updateStatusIfSending(pendingId, cid, 'queued');
        log.error('Caption send failed, queued for offline:', e);
      });
  }

  async function confirmSendFile(caption = '') {
    if (!pendingFiles.length || !contactId || sendingFile) return;
    const files = pendingFiles;
    const cid = contactId;
    const text = caption.trim();
    if (text.length > MAX_MESSAGE_LENGTH) {
      notifications.push(
        t('chat.conversation.errorMessageTooLong', {
          length: text.length,
          max: MAX_MESSAGE_LENGTH,
        }),
        'error'
      );
      return;
    }
    messagesState.markRead(cid, true);
    sendingFile = true;
    try {
      for (const file of files) {
        // The core moves staged bytes out of the pending dir, so the preview
        // has to point at the copy it kept, not the path we handed it.
        const [messageId, fileSize, storedPath] = await sendFileOffer(cid, file.backendPath);
        messagesState.appendOptimistic({
          id: messageId,
          contactId: cid,
          direction: 'sent',
          content: t('chat.conversation.filePlaceholder'),
          status: 'sending',
          timestamp: Date.now(),
          receivedTimestamp: Date.now(),
          replyTo: null,
          fileDetails: {
            filename: file.filename,
            sizeBytes: fileSize,
            autodownloadPath: storedPath || file.backendPath,
          },
        });
        if (storedPath) messagesState.setAutodownloadPath(messageId, cid, storedPath);
        pendingFiles = pendingFiles.filter((f) => f.backendPath !== file.backendPath);
      }
      if (text) sendCaption(cid, text);
      releaseSharedFiles();
      winstonTips.show('fileOffer');
    } catch (e) {
      notifyError(e, 'chat.conversation.errorSendFile');
    } finally {
      sendingFile = false;
    }
  }

  function cancelSendFile() {
    if (sendingFile) return;
    pendingFiles = [];
    releaseSharedFiles();
  }

  function removePendingFile(backendPath: string) {
    if (sendingFile) return;
    pendingFiles = pendingFiles.filter((f) => f.backendPath !== backendPath);
    releaseSharedFiles();
  }

  async function handleAcceptIncomingFile(msg: MessageResponse) {
    if (!contactId || !msg.fileDetails) return;
    if (fileOfferActionState[msg.id] === 'accepting' || fileOfferActionState[msg.id] === 'accepted')
      return;
    fileOfferActionState[msg.id] = 'accepting';
    try {
      const savePath = await resolveDownloadPath(msg.contactId, msg.id, msg.fileDetails.filename);
      await acceptFileOffer(msg.contactId, msg.id, savePath);
      messagesState.setAutodownloadPath(msg.id, msg.contactId, savePath);
      fileOfferActionState[msg.id] = 'accepted';
    } catch (e) {
      fileOfferActionState[msg.id] = 'idle';
      notifications.push(
        t('chat.conversation.errorAcceptFile', { error: parseError(e).message }),
        'error'
      );
      log.error('Accept file offer failed', e);
    }
  }

  // Downloads never prompt, so this is how a file leaves the app: the only
  // route out on mobile, where app storage isn't browsable.
  async function handleSaveToDevice(msg: MessageResponse) {
    const path = msg.fileDetails?.autodownloadPath;
    if (!path || !msg.fileDetails) return;
    try {
      const saved = await exportToDevice(path, msg.fileDetails.filename);
      if (saved) messageFlash.trigger(msg.id);
    } catch (e) {
      notifyError(e, 'chat.conversation.errorExportFile');
    }
  }

  async function handleCancelFile(msg: MessageResponse) {
    if (!msg.fileDetails) return;
    try {
      await cancelFileTransfer(msg.contactId, msg.id);
    } catch (e) {
      log.error('Cancel file transfer failed', e);
    }
    fileOfferActionState[msg.id] = 'idle';
    messagesState.clearTransferProgress(msg.id);
    messagesState.setAutodownloadPath(msg.id, msg.contactId, null);
  }

  // Backend already retries offline sends automatically (on reconnect, every 10
  // min, and across restarts). This just nudges it to republish right now; the
  // existing `delivery_confirmed` event flow takes over from there.
  async function flushQueue(cid: string) {
    if (!cid || flushingQueue) return;
    flushingQueue = true;
    try {
      await flushOffline(cid);
    } catch (e) {
      notifications.push(t('chat.offlineQueue.retryError'), 'error');
      log.error('Queue flush: backend retry failed', e);
    } finally {
      flushingQueue = false;
    }
  }

  function autoFlushQueue(cid: string) {
    if (!cid || flushingQueue) return;
    if (messagesState.queuedFor(cid).length === 0) return;
    void flushOffline(cid).catch((e) => log.error('Auto-flush failed', cid, e));
  }

  $effect(() => {
    const leaving = contactId;
    return () => autoFlushQueue(leaving);
  });

  // Uploads on its own once the queue has been idle for OFFLINE_FLUSH_DELAY.
  // Every new queued item re-runs this effect, which cancels the pending timer
  // and starts a fresh one, so a burst of messages uploads as one batch.
  $effect(() => {
    const cid = contactId;
    if (!cid || queueActivity === 0) return;
    const timer = setTimeout(() => void flushQueue(cid), OFFLINE_FLUSH_DELAY);
    return () => clearTimeout(timer);
  });

  function onBubbleTouchStart(e: TouchEvent, msg: MessageResponse) {
    if (!isCoarsePointer) return;
    // Swipe-to-reply is the touch equivalent of the hover menu's reply button,
    // so it follows the same rule about what is actionable.
    if (!isMessageActionable(msg.status)) return;
    const t = e.touches[0];
    swipeStart = { x: t.clientX, y: t.clientY, id: msg.id };
  }
  // Bubbles are swiped inward, away from the edge they hang off: sent bubbles
  // sit right so they travel left, everything else travels right. Flat layout
  // has no edge alignment, so it keeps the single rightward direction.
  function swipeDirFor(msg: MessageResponse): 1 | -1 {
    return appearanceState.layout !== 'flat' && msg.direction === 'sent' ? -1 : 1;
  }
  function onBubbleTouchMove(e: TouchEvent, msg: MessageResponse) {
    if (!isCoarsePointer || swipeStart.id !== msg.id) return;
    const t = e.touches[0];
    const dx = t.clientX - swipeStart.x;
    const dy = Math.abs(t.clientY - swipeStart.y);
    if (Math.abs(dx) > 6 || dy > 6) cancelLongPress();
    const dir = swipeDirFor(msg);
    const travel = dx * dir;
    if (travel > 10 && dy < 30) {
      swipeOffset = { id: msg.id, dx: Math.min(travel, 80) * dir };
    }
  }
  function onBubbleTouchEnd(msgId: string, msg: MessageResponse) {
    if (swipeOffset?.id === msgId && Math.abs(swipeOffset.dx) > 60) {
      void haptics.impact('medium');
      startReply(msg);
    }
    swipeStart = { x: 0, y: 0, id: '' };
    swipeOffset = null;
  }

  const STACK_MIN = 4;

  function isStackableImage(m: MessageResponse): boolean {
    if (!m.fileDetails) return false;
    if (mediaKindFromFilename(m.fileDetails.filename) !== 'image') return false;
    if (m.replyTo || m.pinned) return false;
    if (m.status === 'failed') return false;
    if (messagesState.reactionsFor(m.id, m.contactId).length > 0) return false;
    return true;
  }

  function imageRuns(msgs: MessageResponse[]): { msgs: MessageResponse[]; startIdx: number }[] {
    const runs: { msgs: MessageResponse[]; startIdx: number }[] = [];
    let i = 0;
    while (i < msgs.length) {
      if (isStackableImage(msgs[i])) {
        let j = i;
        while (j < msgs.length && isStackableImage(msgs[j])) j++;
        if (j - i >= STACK_MIN) {
          runs.push({ msgs: msgs.slice(i, j), startIdx: i });
          i = j;
          continue;
        }
      }
      runs.push({ msgs: [msgs[i]], startIdx: i });
      i++;
    }
    return runs;
  }

  // Accepts every not-yet-downloaded image in a stack. Like a single download
  // this prompts for nothing: each file lands in the app's download folder.
  async function downloadStack(msgs: MessageResponse[]) {
    if (!contactId) return;
    const items = msgs
      .filter((m) => m.fileDetails && fileOfferActionState[m.id] !== 'accepting')
      .map((m) => ({ msg: m, filename: m.fileDetails!.filename }));
    if (items.length === 0) return;

    for (const { msg } of items) fileOfferActionState[msg.id] = 'accepting';

    let failed = 0;
    for (const { msg, filename } of items) {
      try {
        const savePath = await resolveDownloadPath(msg.contactId, msg.id, filename);
        await acceptFileOffer(msg.contactId, msg.id, savePath);
        messagesState.setAutodownloadPath(msg.id, msg.contactId, savePath);
        fileOfferActionState[msg.id] = 'accepted';
      } catch (e) {
        failed += 1;
        fileOfferActionState[msg.id] = 'idle';
        log.error('Download all: accept failed', e);
      }
    }
    if (failed > 0) {
      notifications.push(t('chat.conversation.errorDownloadAll', { count: failed }), 'error');
    }
  }

  // Opens the viewer scoped to the stack the tapped image belongs to. The
  // filmstrip/prev-next only walk this batch's already-downloaded images.
  function openStackImage(m: MessageResponse, stack: MessageResponse[]) {
    const items: MediaItem[] = [];
    let index = -1;
    for (const s of stack) {
      const item = mediaItemFor(s);
      if (!item) continue;
      if (s.id === m.id) index = items.length;
      items.push(item);
    }
    if (index < 0) return;
    mediaViewer = { items, index };
  }

  let longPressTimer: ReturnType<typeof setTimeout> | null = null;
  function onBubbleTouchStartLongPress(msgId: string) {
    if (!isCoarsePointer) return;
    const m = messageIndex.get(msgId);
    if (m && (m.status === 'sending' || m.status === 'failed')) return;
    if (longPressTimer) clearTimeout(longPressTimer);
    longPressTimer = setTimeout(() => {
      actionSheetMsgId = msgId;
      void haptics.impact('medium');
    }, 450);
  }
  function cancelLongPress() {
    if (longPressTimer) {
      clearTimeout(longPressTimer);
      longPressTimer = null;
    }
  }

  function swipeOffsetFor(id: string): number {
    return swipeOffset?.id === id ? swipeOffset.dx : 0;
  }

  const actionSheetMsg = $derived(actionSheetMsgId ? messageIndex.get(actionSheetMsgId) : null);
  const selectTextMsg = $derived(selectTextMsgId ? messageIndex.get(selectTextMsgId) : null);

  function openSelectText(msg: MessageResponse) {
    selectTextMsgId = msg.id;
    actionSheetMsgId = null;
  }
</script>

{#if contact}
  <div class="chat" style="--composer-h: {composerHeight}px;">
    <ChatHeader
      {contact}
      onOpenProfile={openProfileModal}
      onOpenSecurity={openSecurityCodeModal}
      onOpenConnection={openConnectionInfoModal}
      onSearch={openSearch}
    />

    <PinBar
      pinned={messagesState.pinnedFor(contactId)}
      activeId={activePinId}
      onJump={jumpToPinned}
      onUnpin={(id) => {
        messagesState.setPinnedOptimistic(contactId, id, false);
        void pinMessage(contactId, id, false).catch((e) => {
          log.error('unpin failed', e);
          messagesState.setPinnedOptimistic(contactId, id, true);
        });
      }}
    />

    {#if unreadBarShown}
      <UnreadBar
        count={unreadRunCount}
        onJump={() => void jumpToUnread()}
        onMarkRead={markCurrentRead}
      />
    {/if}

    {#if searchOpen}
      <ChatSearchBar
        query={searchQuery}
        count={matchIds.length}
        current={matchPos}
        onInput={(q) => {
          searchQuery = q;
        }}
        onNext={searchNext}
        onPrev={searchPrev}
        onClose={closeSearch}
      />
    {/if}

    {#if showShareBanner}
      <ShareBanner
        name={contact.displayName}
        sharing={shareBusy.active}
        onShare={handleShareQuickProfile}
        onDismiss={closeShareBanner}
      />
    {/if}

    <MessageList
      bind:listEl
      bind:hoveredMessageId
      {contact}
      {messageGroups}
      isEmpty={visibleMessages.length === 0}
      {gapNotices}
      {firstUnreadId}
      {loadingOlder}
      {loadingNewer}
      {sending}
      {terminated}
      onScroll={handleScroll}
      {imageRuns}
      onOpenStackImage={openStackImage}
      onDownloadStack={downloadStack}
      onStackContextMenu={(e, m) => {
        // Touch-only affordance; a mouse right-click has the per-bubble menu.
        if (!isCoarsePointer) return;
        e.preventDefault();
        actionSheetMsgId = m.id;
      }}
      onSayHi={() => {
        inputText = '👋';
        void handleSend();
      }}
      onVerify={openSecurityCodeModal}
      onViewPinned={(id) => void jumpToPinned(id)}
    >
      {#snippet msgBubble(msg: MessageResponse, mi: number, groupLen: number, flatMode: boolean)}
        {@const repliedMessage = msg.replyTo ? (messageIndex.get(msg.replyTo) ?? null) : null}
        {@const reactions = messagesState.reactionsFor(msg.id, msg.contactId)}
        <MessageBubble
          {msg}
          {repliedMessage}
          {reactions}
          syncPending={messagesState.isPendingSync(msg.contactId, msg.id)}
          syncPendingDelete={messagesState.isPendingSyncDelete(msg.contactId, msg.id)}
          layout={flatMode ? 'flat' : 'bubble'}
          userId={profileState.userId || ''}
          peerName={contact.displayName}
          isFirst={mi === 0}
          isLast={mi === groupLen - 1}
          animateIn={msg.timestamp >= chatOpenedAt}
          {isCoarsePointer}
          hovered={hoveredMessageId === msg.id}
          emojiOpen={showEmojiPicker === msg.id}
          searchTerm={searchOpen ? searchQuery : ''}
          swipeDx={swipeOffsetFor(msg.id)}
          fileOfferState={fileOfferActionState[msg.id]}
          flashed={messageFlash.has(msg.id)}
          transferPercent={transferPercentFor(msg.id)}
          transferInProgress={!!messagesState.transferProgressFor(msg.id) &&
            !isTransferDoneFor(msg.id)}
          transferDone={isTransferDoneFor(msg.id)}
          mediaVersion={messagesState.mediaVersionFor(msg.contactId, msg.id)}
          onHoverEnter={flatMode ? () => {} : () => (hoveredMessageId = msg.id)}
          onHoverLeave={flatMode ? () => {} : () => (hoveredMessageId = null)}
          onTouchStart={(e) => {
            onBubbleTouchStart(e, msg);
            onBubbleTouchStartLongPress(msg.id);
          }}
          onTouchMove={(e) => onBubbleTouchMove(e, msg)}
          onTouchEnd={() => {
            onBubbleTouchEnd(msg.id, msg);
            cancelLongPress();
          }}
          onTouchCancel={() => {
            swipeStart = { x: 0, y: 0, id: '' };
            swipeOffset = null;
            cancelLongPress();
          }}
          onContextMenu={(e) => {
            if (!isMessageActionable(msg.status)) return;
            e.preventDefault();
            actionSheetMsgId = msg.id;
          }}
          onReplyRefClick={handleReplyRefClick}
          onAcceptFile={() => handleAcceptIncomingFile(msg)}
          onCancelFile={() => handleCancelFile(msg)}
          onSaveToDevice={() => handleSaveToDevice(msg)}
          onToggleReact={(emoji) => toggleReaction(msg, emoji)}
          onStartReply={() => startReply(msg)}
          onCopy={() => copyMessageText(msg)}
          onStartEdit={() => startEdit(msg)}
          canMarkUnread={canMarkUnread(msg)}
          onTogglePin={() => togglePin(msg)}
          onMarkUnread={() => markUnreadFrom(msg)}
          onForward={() => (forwardContent = msg.content)}
          onDelete={() => handleDelete(msg)}
          onDeleteLocal={() => handleDeleteLocal(msg)}
          onRetry={() => handleRetry(msg)}
          onToggleEmojiPicker={(rect) => {
            if (showEmojiPicker === msg.id) {
              showEmojiPicker = null;
              emojiPickerAnchor = null;
            } else {
              showEmojiPicker = msg.id;
              emojiPickerAnchor = rect;
            }
          }}
          onOpenMedia={(path, kind, filename) => {
            mediaViewer = {
              items: [
                {
                  src: mediaUrl(path, messagesState.mediaVersionFor(msg.contactId, msg.id)),
                  path,
                  kind,
                  filename,
                },
              ],
              index: 0,
            };
          }}
        />
      {/snippet}
    </MessageList>

    {#if visibleMessages.length > 0 && (farFromBottom || !messagesState.isNewestReached(contactId))}
      <ScrollToBottomButton
        {unreadCount}
        name={contact.displayName}
        avatar={contact.avatarBase64}
        onClick={jumpToLatest}
      />
    {/if}

    <DelayedMessagesButton count={delayedCount} onClick={jumpToEarliestDelayed} />

    <div class="composer-host" bind:this={composerHostEl}>
      <OfflineQueueBar
        pendingUpload={pendingUploadCount}
        inMailbox={inMailboxCount}
        flushing={flushingQueue}
        contactName={contact?.displayName ?? ''}
      />

      <MessageComposer
        {contact}
        {terminated}
        bind:inputText
        {sending}
        {isCoarsePointer}
        replyingPreview={replyingToPreview}
        {editingPreview}
        replyActive={!!replyingToMessageId}
        editActive={!!editingMessageId}
        onSend={handleSend}
        onAttach={openAttach}
        onInput={onComposerInput}
        onCancelReply={cancelReply}
        onCancelEdit={cancelEdit}
        onEditLast={editLastMessage}
        onOpenProfile={openProfileModal}
        onPasteImage={handlePasteImage}
        onPasteLongText={(text) => void offerTextAsFile(text)}
        bind:composerEl
      />
    </div>
  </div>

  {#if showAttachSheet}
    <AttachSheet
      onClose={() => (showAttachSheet = false)}
      onPickFiles={handlePickedFiles}
      onPickNative={(mode) => void handleSendFile(mode)}
    />
  {/if}

  {#if actionSheetMsg}
    <ActionSheet
      msg={actionSheetMsg}
      canMarkUnread={canMarkUnread(actionSheetMsg)}
      onClose={() => (actionSheetMsgId = null)}
      onReact={(emoji) => actionSheetMsg && toggleReaction(actionSheetMsg, emoji)}
      onMoreEmoji={() => {
        if (actionSheetMsg) {
          showEmojiPicker = actionSheetMsg.id;
          actionSheetMsgId = null;
        }
      }}
      onReply={() => actionSheetMsg && startReply(actionSheetMsg)}
      onCopy={() => actionSheetMsg && copyMessageText(actionSheetMsg)}
      onSelectText={() => actionSheetMsg && openSelectText(actionSheetMsg)}
      onEdit={() => actionSheetMsg && startEdit(actionSheetMsg)}
      onTogglePin={() => {
        if (actionSheetMsg) {
          togglePin(actionSheetMsg);
          actionSheetMsgId = null;
        }
      }}
      onMarkUnread={() => {
        if (actionSheetMsg) {
          markUnreadFrom(actionSheetMsg);
          actionSheetMsgId = null;
        }
      }}
      onForward={() => {
        if (actionSheetMsg) {
          forwardContent = actionSheetMsg.content;
          actionSheetMsgId = null;
        }
      }}
      onDelete={() => actionSheetMsg && handleDelete(actionSheetMsg)}
    />
  {/if}

  {#if forwardContent}
    <ForwardModal content={forwardContent} onClose={() => (forwardContent = null)} />
  {/if}

  {#if selectTextMsg}
    <SelectTextModal text={selectTextMsg.content} onClose={() => (selectTextMsgId = null)} />
  {/if}

  {#if showSecurityCode && contact}
    <SecurityCodeModal
      contactId={contact.userId}
      contactVerified={contact.verified}
      onClose={closeSecurityCodeModal}
    />
  {/if}
  {#if showConnectionInfo && contact}
    <ConnectionInfoModal {contact} onClose={closeConnectionInfoModal} />
  {/if}
  {#if showProfile && contact}
    <ProfileModal {contact} onClose={closeProfileModal} />
  {/if}

  {#if isDraggingFile}
    <div class="drop-overlay" aria-hidden="true">
      <div class="drop-overlay-inner">
        <Paperclip size={36} />
        <span>{t('chat.conversation.dropFileOverlay')}</span>
      </div>
    </div>
  {/if}

  {#if showEmojiPicker}
    <div
      class="emoji-picker-layer"
      style={emojiPickerPos ? emojiPickerLayerStyle(emojiPickerPos) : PICKER_CENTER_STYLE}
    >
      <EmojiPicker
        compact
        onSelect={handlePickerSelect}
        onClose={closePicker}
        autoFocus={!isCoarsePointer}
      />
    </div>
  {/if}

  {#if mediaViewer}
    {@const cur = mediaViewer.items[mediaViewer.index]}
    <MediaViewer
      src={cur.src}
      path={cur.path}
      kind={cur.kind}
      filename={cur.filename}
      items={mediaViewer.items}
      index={mediaViewer.index}
      onSelect={(i) => mediaViewer && (mediaViewer = { ...mediaViewer, index: i })}
      onClose={() => (mediaViewer = null)}
      onPrev={mediaPrev}
      onNext={mediaNext}
      hasPrev={mediaViewer.index > 0}
      hasNext={mediaViewer.index < mediaViewer.items.length - 1}
    />
  {/if}

  {#if pendingFiles.length > 0}
    <FileConfirmModal
      files={pendingFiles}
      sending={sendingFile}
      maxLength={MAX_MESSAGE_LENGTH}
      initialCaption={shareCaption}
      onConfirm={confirmSendFile}
      onCancel={cancelSendFile}
      onRemove={removePendingFile}
    />
  {/if}
{:else}
  <div class="loading"><Spinner /></div>
{/if}

<style>
  .chat {
    --chat-max: 1000px;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    position: relative;
    background: transparent;
    animation: chat-appear 0.18s ease both;
  }
  @keyframes chat-appear {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .composer-host {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    max-width: var(--chat-max);
    margin-inline: auto;
    padding: 0 max(8px, var(--safe-right)) max(8px, var(--safe-bottom), var(--kb-overlap, 0px))
      max(8px, var(--safe-left));
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
    pointer-events: none;
    z-index: 12;
  }
  .composer-host::before {
    content: '';
    position: absolute;
    inset: -14px 0 0 0;
    z-index: -1;
    pointer-events: none;
    background: linear-gradient(to top, var(--bg-secondary), transparent);
  }
  .composer-host > :global(*) {
    pointer-events: auto;
  }

  .loading {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .emoji-picker-layer {
    position: fixed;
    z-index: 250;
    animation: pickerFadeIn 0.12s ease;
  }
  @keyframes pickerFadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .drop-overlay {
    position: absolute;
    inset: 0;
    background: var(--surface);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 220;
    pointer-events: none;
    animation: fadeIn 0.12s ease;
  }
  /* Translucency only where color-mix() exists: a color-mix() value containing
     var() computes to `transparent` on engines without it (Chrome < 111). */
  @supports (background: color-mix(in srgb, red 50%, transparent)) {
    .drop-overlay {
      background: color-mix(in srgb, var(--surface) 85%, transparent);
    }
  }
  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  .drop-overlay-inner {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 28px 40px;
    border: 2px dashed var(--accent);
    border-radius: var(--radius-lg);
    background: var(--accent-dim);
    color: var(--accent);
    font-weight: 600;
    font-size: 14px;
  }
</style>
