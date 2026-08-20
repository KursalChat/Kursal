import type { MessageResponse } from '$lib/types';
import { log } from '$lib/utils/log';
import { flushContact, pendingSyncKey } from '$lib/utils/pendingSync';
import {
  getMessages,
  getMessagesAfter,
  getMessagesAround,
  getPinnedMessages,
  retryMessage as retryMessageApi,
  sendReadReceipts,
} from '$lib/api/messages';
import {
  getDelayedUnseen,
  getPendingSync,
  getUnreadSummary,
  markContactRead,
  markContactUnread,
  setContactMarkedUnread,
  setDelayedUnseen,
} from '$lib/api/conversation';
import { clearNotificationsFor } from '$lib/api/system-notify';
import { insertInSentOrder } from './delayedStore';

const SEND_TIMEOUT_MS = 15_000;
const READ_COMMIT_DEBOUNCE_MS = 400;
const DELAYED_PERSIST_DEBOUNCE_MS = 300;

// Renderable messages have text content or are structured events (call and pin
// records) whose body lives in a typed field rather than `content`.
const hasRenderableBody = (m: MessageResponse) =>
  m.content !== '' || !!m.callDetails || !!m.pinDetails;

function createMessagesState() {
  let map = $state<Record<string, MessageResponse[]>>({});
  // Counts are a live mirror of what the core derives from each contact's read
  // cursor: seeded by `hydrate`, then kept in step as messages arrive. Nothing
  // is written back per message; only the user's own read/unread actions move
  // the cursor.
  let unreadByContact = $state<Record<string, number>>({});
  // Contacts whose count stopped at the core's scan cap, rendered as "99+".
  let unreadCapped = $state<Record<string, boolean>>({});
  // The id of the first message that arrived while the chat wasn't
  // actively viewed. Used to draw a "New messages" separator.
  let firstUnreadByContact = $state<Record<string, string>>({});
  // Conversations the user put back to unread by hand. Reading actions are
  // suppressed for these until the chat is left and reopened, otherwise the
  // at-bottom auto-read would undo the click in the same frame.
  let markedUnread = $state<Set<string>>(new Set());
  let reactions = $state<Record<string, Array<{ emoji: string; userIds: string[] }>>>({});
  // Pinned messages, sourced from the backend pinned index.
  let pinnedByContact = $state<Record<string, MessageResponse[]>>({});
  // messageIds hidden optimistically between the delete click and its commit
  let pendingDelete = $state<Set<string>>(new Set());
  let transferProgress = $state<Record<string, { bytesTransferred: number; totalBytes: number }>>(
    {}
  );
  // Bumped when a received file finishes writing. The <img>/<video> mounts against
  // the still-empty preallocated file, so the first fetch fails; folding this into
  // the media URL forces a refetch once the bytes are actually on disk.
  let mediaVersions = $state<Record<string, number>>({});
  let loadedContacts = $state<Set<string>>(new Set());
  // Tail timestamp of a window that has been evicted, so sidebar ordering
  // doesn't reset to 0 for conversations dropped from memory.
  let lastTs = $state<Record<string, number>>({});
  // `message_queued_offline` events can beat the id swap (replaceId/append);
  // buffered here until the id exists.
  const pendingQueued: Set<string> = new Set();
  function pendingQueuedKey(contactId: string, messageId: string) {
    return `${contactId}:${messageId}`;
  }
  // Messages that took the offline path, so `delivery_confirmed` promotes to
  // `offline_delivered` instead of `delivered`. Session-only.
  const viaOffline: Set<string> = new Set();
  // Offline mailbox counters the backend gave up on (48h gap-skip); rendered
  // as a greyed notice. Session-only.
  let gapNoticesByContact = $state<Record<string, { counter: number; timestamp: number }[]>>({});
  // Received messages whose sent-time placed them above the live tail (delayed
  // offline delivery), so they'd be easy to miss. Persisted per contact,
  // cleared once the message is scrolled into view.
  let delayedUnseen = $state<Record<string, string[]>>({});

  type TimerMap = Map<string, ReturnType<typeof setTimeout>>;
  const readTimers: TimerMap = new Map();
  const delayedTimers: TimerMap = new Map();

  function debounceFor(timers: TimerMap, contactId: string, ms: number, run: () => void) {
    clearTimeout(timers.get(contactId));
    timers.set(
      contactId,
      setTimeout(() => {
        timers.delete(contactId);
        run();
      }, ms)
    );
  }

  // A pending timer outlives the contact it belongs to and would write state
  // back after the core already dropped it.
  function cancelTimers(contactId?: string) {
    for (const timers of [readTimers, delayedTimers]) {
      if (contactId === undefined) {
        timers.forEach(clearTimeout);
        timers.clear();
        continue;
      }
      clearTimeout(timers.get(contactId));
      timers.delete(contactId);
    }
  }

  function persistDelayed(contactId: string) {
    debounceFor(delayedTimers, contactId, DELAYED_PERSIST_DEBOUNCE_MS, () => {
      void setDelayedUnseen(contactId, delayedUnseen[contactId] ?? []).catch((e) =>
        log.error('Failed to persist delayed-unseen for', contactId, e)
      );
    });
  }
  function addDelayed(contactId: string, id: string) {
    const cur = delayedUnseen[contactId] ?? [];
    if (cur.includes(id)) return;
    delayedUnseen = { ...delayedUnseen, [contactId]: [...cur, id] };
    persistDelayed(contactId);
  }
  function delayedUnseenFor(contactId: string): string[] {
    return delayedUnseen[contactId] ?? [];
  }
  function markDelayedSeen(contactId: string, id: string) {
    const cur = delayedUnseen[contactId];
    if (!cur || !cur.includes(id)) return;
    delayedUnseen = { ...delayedUnseen, [contactId]: cur.filter((x) => x !== id) };
    persistDelayed(contactId);
  }

  function reactionKey(contactId: string, messageId: string) {
    return `${contactId}:${messageId}`;
  }

  const PAGE_SIZE = 50;
  // Upper bound on a contact's loaded window.
  const WINDOW_MAX = 300;
  // A contact is "at live tail" when its loaded window includes the newest
  // message. After a jump (loadAround) it isn't, until scrolled/loaded back.
  const oldestReached: Set<string> = new Set();
  const newestReached: Set<string> = new Set();

  function trimTail(contactId: string) {
    const list = map[contactId];
    if (!list || list.length <= WINDOW_MAX) return;
    list.splice(WINDOW_MAX);
    newestReached.delete(contactId);
  }

  function trimHead(contactId: string) {
    const list = map[contactId];
    if (!list || list.length <= WINDOW_MAX) return;
    list.splice(0, list.length - WINDOW_MAX);
    oldestReached.delete(contactId);
  }

  function processMessage(m: MessageResponse) {
    if (m.reactions && m.reactions.length > 0) {
      const key = reactionKey(m.contactId, m.id);
      const grouped: Record<string, { emoji: string; userIds: string[] }> = {};
      m.reactions.forEach((r) => {
        if (!grouped[r.emoji]) grouped[r.emoji] = { emoji: r.emoji, userIds: [] };
        if (!grouped[r.emoji].userIds.includes(r.userId)) {
          grouped[r.emoji].userIds.push(r.userId);
        }
      });
      reactions[key] = Object.values(grouped);
    }
    // The backend derives queued / queued_in_dht / offline_delivered
    // authoritatively (see dto::apply_offline_overlay). Re-seed `viaOffline` so
    // a later live `delivery_confirmed` still promotes to `offline_delivered`.
    if (m.status === 'queued' || m.status === 'queued_in_dht' || m.status === 'offline_delivered') {
      viaOffline.add(pendingQueuedKey(m.contactId, m.id));
    }
  }

  function applyPendingQueued(contactId: string) {
    const prefix = `${contactId}:`;
    for (const key of Array.from(pendingQueued)) {
      if (key.startsWith(prefix)) {
        consumePendingQueued(contactId, key.slice(prefix.length));
      }
    }
  }

  // Loads (or reloads) the newest page: the live tail. Reloads a contact that
  // was previously left in a jumped (non-tail) window.
  async function loadFor(contactId: string) {
    void loadPinned(contactId);
    if (loadedContacts.has(contactId) && newestReached.has(contactId)) return;

    // On failure, stay uncached so the next open/effect run retries, rather
    // than freezing the chat empty for the session.
    let msgs: MessageResponse[];
    try {
      msgs = await getMessages(contactId, PAGE_SIZE);
    } catch (e) {
      log.error('Failed to load messages for', contactId, e);
      return;
    }

    if (msgs.length < PAGE_SIZE) oldestReached.add(contactId);
    else oldestReached.delete(contactId);
    newestReached.add(contactId);
    msgs.forEach(processMessage);
    map[contactId] = msgs.filter(hasRenderableBody);
    applyPendingQueued(contactId);
    loadedContacts.add(contactId);
  }

  // Prepends the previous page of messages (those just above the oldest loaded).
  async function loadOlder(contactId: string): Promise<number> {
    const list = map[contactId];
    if (!list || list.length === 0 || oldestReached.has(contactId)) return 0;
    try {
      const older = await getMessages(contactId, PAGE_SIZE, list[0].id);
      if (older.length < PAGE_SIZE) oldestReached.add(contactId);
      older.forEach(processMessage);
      const existing = new Set(list.map((m) => m.id));
      const toPrepend = older.filter((m) => hasRenderableBody(m) && !existing.has(m.id));
      if (toPrepend.length > 0) {
        map[contactId] = [...toPrepend, ...list];
        trimTail(contactId);
      }
      return toPrepend.length;
    } catch (e) {
      log.error('Failed to load older messages for', contactId, e);
      return 0;
    }
  }

  // Appends the next page of newer messages (below the newest loaded). Used when
  // scrolling down through a window that isn't yet at the live tail.
  async function loadNewer(contactId: string): Promise<number> {
    const list = map[contactId];
    if (!list || list.length === 0 || newestReached.has(contactId)) return 0;
    try {
      const newer = await getMessagesAfter(contactId, list[list.length - 1].id, PAGE_SIZE);
      if (newer.length < PAGE_SIZE) newestReached.add(contactId);
      newer.forEach(processMessage);
      const existing = new Set(list.map((m) => m.id));
      const toAppend = newer.filter((m) => hasRenderableBody(m) && !existing.has(m.id));
      if (toAppend.length > 0) {
        map[contactId] = [...list, ...toAppend];
        trimHead(contactId);
      }
      return toAppend.length;
    } catch (e) {
      log.error('Failed to load newer messages for', contactId, e);
      return 0;
    }
  }

  // Replaces the window with one centered on `messageId` (jump to any message).
  // Ends are unknown afterwards and discovered by scrolling (loadOlder/loadNewer).
  async function loadAround(contactId: string, messageId: string): Promise<boolean> {
    try {
      const win = await getMessagesAround(contactId, messageId, PAGE_SIZE);
      if (win.length === 0) return false;
      win.forEach(processMessage);
      map[contactId] = win.filter(hasRenderableBody);
      oldestReached.delete(contactId);
      newestReached.delete(contactId);
      loadedContacts.add(contactId);
      return (map[contactId] ?? []).some((m) => m.id === messageId);
    } catch (e) {
      log.error('Failed to load around message for', contactId, e);
      return false;
    }
  }

  function isOldestReached(contactId: string): boolean {
    return oldestReached.has(contactId);
  }

  function isNewestReached(contactId: string): boolean {
    return newestReached.has(contactId);
  }

  function forContact(contactId: string): MessageResponse[] {
    const list = map[contactId] ?? [];
    return pendingDelete.size > 0 ? list.filter((m) => !pendingDelete.has(m.id)) : list;
  }

  // 0 when the conversation has never been opened.
  function lastTimestampFor(contactId: string): number {
    const list = map[contactId];
    if (list?.length) return list[list.length - 1].timestamp;
    return lastTs[contactId] ?? 0;
  }

  function noteLastTs(contactId: string) {
    const list = map[contactId];
    const ts = list?.length ? list[list.length - 1].timestamp : 0;
    if (ts > (lastTs[contactId] ?? 0)) lastTs[contactId] = ts;
  }

  function evictOthers(keepId: string) {
    const drop = Object.keys(map).filter((cid) => cid !== keepId);
    if (drop.length === 0) return;
    for (const cid of drop) {
      noteLastTs(cid);
      delete map[cid];
      oldestReached.delete(cid);
      newestReached.delete(cid);
    }
    loadedContacts = new Set([...loadedContacts].filter((id) => !drop.includes(id)));
  }

  function setPendingDelete(messageId: string, pending: boolean) {
    const next = new Set(pendingDelete);
    if (pending) next.add(messageId);
    else next.delete(messageId);
    pendingDelete = next;
  }

  function markMediaReady(contactId: string, messageId: string) {
    const key = `${contactId}:${messageId}`;
    mediaVersions[key] = (mediaVersions[key] ?? 0) + 1;
  }

  function mediaVersionFor(contactId: string, messageId: string): number {
    return mediaVersions[`${contactId}:${messageId}`] ?? 0;
  }

  // Edits/reactions/deletes made while the peer is offline, shown with a "waiting
  // to sync" clock until `offline_queue_drained`. `pendingSyncDelete` tracks the
  // delete subset as a tombstone. The core records both when it queues the
  // change; these mirror it so the clock appears without a round trip.
  let pendingSync = $state<Set<string>>(new Set());
  let pendingSyncDelete = $state<Set<string>>(new Set());

  function markPendingSync(contactId: string, messageId: string, isDelete = false) {
    const key = pendingSyncKey(contactId, messageId);
    const next = new Set(pendingSync);
    next.add(key);
    pendingSync = next;
    if (isDelete) {
      const nd = new Set(pendingSyncDelete);
      nd.add(key);
      pendingSyncDelete = nd;
    }
  }
  function isPendingSync(contactId: string, messageId: string): boolean {
    return pendingSync.has(pendingSyncKey(contactId, messageId));
  }
  // Edits/reactions/deletes waiting on the same offline queue as `queued`
  // messages, so the auto-flush timer has to see them move too.
  function pendingSyncCountFor(contactId: string): number {
    const prefix = `${contactId}:`;
    let n = 0;
    for (const key of pendingSync) if (key.startsWith(prefix)) n++;
    return n;
  }
  function isPendingSyncDelete(contactId: string, messageId: string): boolean {
    return pendingSyncDelete.has(pendingSyncKey(contactId, messageId));
  }
  function flushPendingSync(contactId: string, finalizedDeletes: string[] = []) {
    const result = flushContact({ sync: pendingSync, deleted: pendingSyncDelete }, contactId);
    if (result.changed) {
      pendingSync = result.state.sync;
      pendingSyncDelete = result.state.deleted;
    }
    for (const messageId of finalizedDeletes) {
      removeMessage(messageId, contactId);
    }
  }

  function append(msg: MessageResponse) {
    if (!hasRenderableBody(msg)) return;
    const cid = msg.contactId;
    // Viewing a jumped (non-tail) window: don't inject new messages into it;
    // they'd appear out of place. Still count them as unread.
    if (loadedContacts.has(cid) && !newestReached.has(cid)) {
      if (msg.direction === 'received') {
        unreadByContact[cid] = (unreadByContact[cid] ?? 0) + 1;
      }
      return;
    }
    if (!map[cid]) map[cid] = [];
    const list = map[cid];
    if (!list.find((m) => m.id === msg.id)) {
      // Insert by sent-time so a delayed offline message lands at its true
      // position, and live order matches reloaded order.
      const idx = insertInSentOrder(list, msg);
      if (msg.direction === 'received') {
        unreadByContact[cid] = (unreadByContact[cid] ?? 0) + 1;
        // Not at the tail => it slotted into history and is easy to miss.
        if (idx < list.length - 1) addDelayed(cid, msg.id);
      }
    }
    // Cover the case where the queued event arrives before the message is
    // appended (e.g. on chat-load with a still-pending sending message).
    consumePendingQueued(cid, msg.id);
  }

  function appendOptimistic(msg: MessageResponse) {
    if (!map[msg.contactId]) map[msg.contactId] = [];
    map[msg.contactId].push(msg);
    // Replying is proof of presence: drop the hand-set unread so the normal
    // at-bottom auto-read can take over again.
    if (msg.direction === 'sent') {
      clearMarkedUnread(msg.contactId);
      clearFirstUnread(msg.contactId);
    }
    // File offers append with their real backend id, so a queued event that
    // raced ahead of this append may already be buffered; apply it now.
    // (Text sends use a temp UUID; replaceId consumes for those.)
    consumePendingQueued(msg.contactId, msg.id);
    const ts = msg.timestamp;
    const cid = msg.contactId;
    setTimeout(() => expireSendingAt(cid, ts), SEND_TIMEOUT_MS);
  }

  // Mark any "sending" message in `contactId` with timestamp <= `beforeTs`
  // as "queued". Keyed by timestamp (not id) so it survives `replaceId`
  // swapping the optimistic UUID for the backend-issued real id.
  function expireSendingAt(contactId: string, beforeTs: number) {
    const list = map[contactId];
    if (!list) return;
    for (const m of list) {
      if (m.status === 'sending' && m.timestamp <= beforeTs) {
        m.status = 'queued';
        viaOffline.add(pendingQueuedKey(contactId, m.id));
      }
    }
  }

  function queuedFor(contactId: string): MessageResponse[] {
    return (map[contactId] ?? []).filter(
      (m) => m.status === 'queued' || m.status === 'queued_in_dht'
    );
  }

  // The two offline stages are distinct: 'queued' is still local and awaiting
  // the batch window, 'queued_in_dht' is already published to the mailbox.
  // Blending them into one count made the queue bar's number unreadable.
  function pendingUploadFor(contactId: string): MessageResponse[] {
    return (map[contactId] ?? []).filter((m) => m.status === 'queued');
  }

  function inMailboxFor(contactId: string): MessageResponse[] {
    return (map[contactId] ?? []).filter((m) => m.status === 'queued_in_dht');
  }

  function queuedCount(contactId: string): number {
    return queuedFor(contactId).length;
  }

  function updateStatus(messageId: string, contactId: string, status: MessageResponse['status']) {
    const list = map[contactId];
    if (!list) return;
    const msg = list.find((m) => m.id === messageId);
    if (msg) {
      // Read is terminal; a late delivery receipt must not downgrade it.
      if (msg.status === 'read' && (status === 'delivered' || status === 'offline_delivered')) {
        return;
      }
      // If the backend says "delivered" but this message took the offline
      // path at some point, promote to `offline_delivered` so the UI can
      // distinguish where the receipt arrived from.
      const promoted =
        status === 'delivered' && viaOffline.has(pendingQueuedKey(contactId, messageId))
          ? 'offline_delivered'
          : status;
      msg.status = promoted;
      // Once delivered, no further "queued" promotion is meaningful.
      if (promoted === 'delivered' || promoted === 'offline_delivered') {
        viaOffline.delete(pendingQueuedKey(contactId, messageId));
      }
    }
  }

  function updateStatusIfSending(
    messageId: string,
    contactId: string,
    status: MessageResponse['status']
  ): boolean {
    const list = map[contactId];
    const msg = list?.find((m) => m.id === messageId);
    if (msg && msg.status === 'sending') {
      msg.status = status;
      if (status === 'queued') {
        viaOffline.add(pendingQueuedKey(contactId, messageId));
      }
      return true;
    }
    // Couldn't apply now (message not loaded yet, or its id is still the
    // optimistic UUID). Buffer "queued" specifically so it lands as soon as
    // `replaceId` / `append` makes the id available.
    if (status === 'queued') {
      pendingQueued.add(pendingQueuedKey(contactId, messageId));
    }
    return false;
  }

  function markBundlePublished(contactId: string, messageIds: string[]) {
    const list = map[contactId];
    if (!list) return;
    for (const id of messageIds) {
      const msg = list.find((m) => m.id === id);
      if (msg && (msg.status === 'queued' || msg.status === 'sending')) {
        msg.status = 'queued_in_dht';
        viaOffline.add(pendingQueuedKey(contactId, id));
      }
    }
  }

  // The backend gave up on these after the offline retry window (3 weeks).
  function markFailed(contactId: string, messageIds: string[]) {
    const list = map[contactId];
    if (!list) return;
    for (const id of messageIds) {
      const msg = list.find((m) => m.id === id);
      if (
        msg &&
        (msg.status === 'queued' || msg.status === 'queued_in_dht' || msg.status === 'sending')
      ) {
        msg.status = 'failed';
      }
    }
  }

  // Re-runs the full send lifecycle for a failed message. The backend reuses the
  // same MessageId, so a duplicate arrival collapses to one message.
  async function retryMessage(contactId: string, messageId: string) {
    const list = map[contactId];
    const msg = list?.find((m) => m.id === messageId);
    if (msg) msg.status = 'sending';
    try {
      await retryMessageApi(contactId, messageId);
    } catch (e) {
      log.error('retry failed', e);
      const failed = map[contactId]?.find((m) => m.id === messageId);
      if (failed) failed.status = 'failed';
    }
  }

  function consumePendingQueued(contactId: string, messageId: string) {
    const key = pendingQueuedKey(contactId, messageId);
    if (!pendingQueued.delete(key)) return;
    const list = map[contactId];
    if (!list) return;
    const msg = list.find((m) => m.id === messageId);
    if (msg && msg.status === 'sending') {
      msg.status = 'queued';
      viaOffline.add(key);
    }
  }

  function replaceId(oldId: string, contactId: string, newId: string) {
    const list = map[contactId];
    if (!list) return;
    const msg = list.find((m) => m.id === oldId);
    if (!msg) return;
    msg.id = newId;
    const oldKey = reactionKey(contactId, oldId);
    if (reactions[oldKey]) {
      const newKey = reactionKey(contactId, newId);
      reactions[newKey] = reactions[oldKey];
      const copy = { ...reactions };
      delete copy[oldKey];
      reactions = copy;
    }
    // If a `message_queued_offline` event landed before this swap, apply it
    // now that the id matches.
    consumePendingQueued(contactId, newId);
  }

  function updateContent(messageId: string, contactId: string, content: string) {
    const list = map[contactId];
    if (!list) return;
    const msg = list.find((m) => m.id === messageId);
    if (msg) {
      msg.content = content;
      msg.edited = true;
    }
  }

  function removePinnedEntry(contactId: string, messageId: string) {
    const pins = pinnedByContact[contactId];
    if (pins && pins.some((m) => m.id === messageId)) {
      pinnedByContact[contactId] = pins.filter((m) => m.id !== messageId);
    }
  }

  function setAutodownloadPath(messageId: string, contactId: string, path: string | null) {
    const list = map[contactId];
    if (!list) return;
    const msg = list.find((m) => m.id === messageId);
    if (!msg || !msg.fileDetails) return;
    msg.fileDetails = { ...msg.fileDetails, autodownloadPath: path };
  }

  function removeMessage(messageId: string, contactId: string) {
    removePinnedEntry(contactId, messageId);
    const list = map[contactId];
    if (!list) return;
    map[contactId] = list.filter((m) => m.id !== messageId);
  }

  function unreadFor(contactId: string): number {
    return unreadByContact[contactId] ?? 0;
  }

  function unreadCappedFor(contactId: string): boolean {
    return unreadCapped[contactId] === true;
  }

  function totalUnread(): number {
    return Object.values(unreadByContact).reduce((acc, n) => acc + n, 0);
  }

  function commitRead(contactId: string) {
    debounceFor(readTimers, contactId, READ_COMMIT_DEBOUNCE_MS, () => {
      void markContactRead(contactId)
        .then((ids) => {
          if (ids.length) return sendReadReceipts(contactId, ids);
        })
        .catch((e) => log.error('Failed to mark read', contactId, e));
    });
  }

  function markRead(contactId: string, force = false) {
    if (!force && markedUnread.has(contactId)) return;
    void clearNotificationsFor(contactId);
    clearMarkedUnread(contactId);
    if (!unreadByContact[contactId]) return;
    unreadByContact[contactId] = 0;
    delete unreadCapped[contactId];
    commitRead(contactId);
  }

  function markAllRead() {
    for (const cid of Object.keys(unreadByContact)) {
      if (!unreadByContact[cid]) continue;
      void clearNotificationsFor(cid);
      unreadByContact[cid] = 0;
      delete unreadCapped[cid];
      commitRead(cid);
    }
    for (const cid of markedUnread) void setContactMarkedUnread(cid, false).catch(() => {});
    markedUnread = new Set();
  }

  // Everything from `messageId` down goes back to unread.
  function applyUnread(contactId: string, fromMessageId: string | null) {
    clearTimeout(readTimers.get(contactId));
    readTimers.delete(contactId);
    void markContactUnread(contactId, fromMessageId)
      .then((entry) => {
        unreadByContact[contactId] = entry.count;
        unreadCapped[contactId] = entry.capped;
        if (entry.firstUnread) firstUnreadByContact[contactId] = entry.firstUnread;
      })
      .catch((e) => log.error('Failed to mark unread', contactId, e));
  }

  function markUnreadFrom(contactId: string, messageId: string) {
    const list = map[contactId];
    if (!list) return;
    const idx = list.findIndex((m) => m.id === messageId);
    if (idx === -1) return;
    let count = 0;
    for (let i = idx; i < list.length; i++) if (list[i].direction === 'received') count++;
    if (count === 0) return;
    unreadByContact[contactId] = count;
    firstUnreadByContact[contactId] =
      list[idx].direction === 'received'
        ? messageId
        : (list.slice(idx).find((m) => m.direction === 'received')?.id ?? messageId);
    markedUnread = new Set(markedUnread).add(contactId);
    applyUnread(contactId, firstUnreadByContact[contactId]);
  }

  function markUnread(contactId: string) {
    const list = map[contactId] ?? [];
    for (let i = list.length - 1; i >= 0; i--) {
      if (list[i].direction !== 'received') continue;
      markUnreadFrom(contactId, list[i].id);
      return;
    }
    if (loadedContacts.has(contactId)) return;
    // Nothing loaded to anchor a separator to; the core picks the newest received message.
    markedUnread = new Set(markedUnread).add(contactId);
    applyUnread(contactId, null);
  }

  function isMarkedUnread(contactId: string): boolean {
    return markedUnread.has(contactId);
  }

  function clearMarkedUnread(contactId: string) {
    if (!markedUnread.has(contactId)) return;
    const next = new Set(markedUnread);
    next.delete(contactId);
    markedUnread = next;
    void setContactMarkedUnread(contactId, false).catch((e) =>
      log.error('Failed to clear marked-unread', contactId, e)
    );
  }

  // Suppression is scoped to the visit that set it, so opening the chat is
  // always what reads it. Leaving keeps the separator the user just placed.
  function enterChat(contactId: string) {
    clearMarkedUnread(contactId);
    evictOthers(contactId);
  }

  function leaveChat(contactId: string) {
    if (markedUnread.has(contactId)) return;
    clearFirstUnread(contactId);
  }

  function markMessagesRead(contactId: string, messageIds: string[]) {
    const list = map[contactId];
    if (!list) return;
    const ids = new Set(messageIds);
    for (const m of list) {
      if (ids.has(m.id) && m.direction === 'sent' && m.status !== 'read') {
        m.status = 'read';
      }
    }
  }

  function firstUnreadFor(contactId: string): string | null {
    return firstUnreadByContact[contactId] ?? null;
  }

  function setFirstUnread(contactId: string, messageId: string) {
    if (firstUnreadByContact[contactId]) return;
    firstUnreadByContact[contactId] = messageId;
  }

  function clearFirstUnread(contactId: string) {
    if (markedUnread.has(contactId)) return;
    if (!firstUnreadByContact[contactId]) return;
    const next = { ...firstUnreadByContact };
    delete next[contactId];
    firstUnreadByContact = next;
  }

  function addReaction(messageId: string, contactId: string, emoji: string, userId: string) {
    const key = reactionKey(contactId, messageId);
    const current = reactions[key] ?? [];
    const index = current.findIndex((r) => r.emoji === emoji);
    if (index >= 0) {
      if (!current[index].userIds.includes(userId)) {
        current[index].userIds.push(userId);
      }
    } else {
      current.push({ emoji, userIds: [userId] });
    }
    reactions[key] = [...current];
  }

  function removeReaction(messageId: string, contactId: string, emoji: string, userId: string) {
    const key = reactionKey(contactId, messageId);
    let current = reactions[key] ?? [];
    const index = current.findIndex((r) => r.emoji === emoji);
    if (index >= 0) {
      current[index].userIds = current[index].userIds.filter((u) => u !== userId);

      if (current[index].userIds.length === 0) {
        current = current.filter((r) => r.emoji !== emoji);
      } else {
        current = [...current];
      }
      reactions[key] = current;
    }
  }

  function reactionsFor(
    messageId: string,
    contactId: string
  ): Array<{ emoji: string; userIds: string[] }> {
    return reactions[reactionKey(contactId, messageId)] ?? [];
  }

  async function loadPinned(contactId: string) {
    try {
      pinnedByContact[contactId] = await getPinnedMessages(contactId);
    } catch (e) {
      log.error('Failed to load pinned for', contactId, e);
    }
  }

  function pinnedFor(contactId: string): MessageResponse[] {
    return pinnedByContact[contactId] ?? [];
  }

  function applyPinned(contactId: string, messageId: string, pinned: boolean) {
    const list = map[contactId];
    if (list) {
      const msg = list.find((m) => m.id === messageId);
      if (msg && msg.pinned !== pinned) msg.pinned = pinned;
    }
    void loadPinned(contactId);
  }

  // Reflects a pin/unpin in the UI immediately, without waiting for the backend
  // round-trip + event + refetch. The authoritative `MessagePinned` event later
  // reconciles via applyPinned -> loadPinned. Callers revert this on failure.
  function setPinnedOptimistic(contactId: string, messageId: string, pinned: boolean) {
    const list = map[contactId];
    const msg = list?.find((m) => m.id === messageId);
    if (msg) msg.pinned = pinned;
    const pins = pinnedByContact[contactId] ? [...pinnedByContact[contactId]] : [];
    const idx = pins.findIndex((m) => m.id === messageId);
    if (pinned) {
      if (idx === -1 && msg) {
        pins.push({ ...msg, pinned: true });
        pins.sort((a, b) => a.timestamp - b.timestamp);
        pinnedByContact[contactId] = pins;
      }
    } else if (idx !== -1) {
      pins.splice(idx, 1);
      pinnedByContact[contactId] = pins;
    }
  }

  function setTransferProgress(transferId: string, bytesTransferred: number, totalBytes: number) {
    transferProgress[transferId] = { bytesTransferred, totalBytes };
  }

  function transferProgressFor(
    transferId: string
  ): { bytesTransferred: number; totalBytes: number } | null {
    return transferProgress[transferId] ?? null;
  }

  function clearTransferProgress(transferId: string) {
    if (!transferProgress[transferId]) return;
    const copy = { ...transferProgress };
    delete copy[transferId];
    transferProgress = copy;
  }

  function hasActiveTransfers(): boolean {
    return Object.values(transferProgress).some(
      (p) => p.totalBytes > 0 && p.bytesTransferred < p.totalBytes
    );
  }

  // Both clears run after the core has already dropped its own copy, so they
  // only have to catch the in-memory state up.
  function clearForContact(contactId: string) {
    cancelTimers(contactId);
    delete map[contactId];
    delete lastTs[contactId];
    delete unreadByContact[contactId];
    delete unreadCapped[contactId];
    delete firstUnreadByContact[contactId];
    delete delayedUnseen[contactId];
    markedUnread = new Set([...markedUnread].filter((id) => id !== contactId));
    delete pinnedByContact[contactId];
    const prefix = `${contactId}:`;
    Object.keys(reactions)
      .filter((k) => k.startsWith(prefix))
      .forEach((k) => delete reactions[k]);
    for (const key of Array.from(pendingQueued)) {
      if (key.startsWith(prefix)) pendingQueued.delete(key);
    }
    for (const key of Array.from(viaOffline)) {
      if (key.startsWith(prefix)) viaOffline.delete(key);
    }
    Object.keys(mediaVersions)
      .filter((k) => k.startsWith(prefix))
      .forEach((k) => delete mediaVersions[k]);
    const flushed = flushContact({ sync: pendingSync, deleted: pendingSyncDelete }, contactId);
    if (flushed.changed) {
      pendingSync = flushed.state.sync;
      pendingSyncDelete = flushed.state.deleted;
    }
    loadedContacts = new Set([...loadedContacts].filter((id) => id !== contactId));
    oldestReached.delete(contactId);
    newestReached.delete(contactId);
    delete gapNoticesByContact[contactId];
  }

  function addGapNotice(contactId: string, counter: number) {
    const list = gapNoticesByContact[contactId] ?? [];
    if (list.some((g) => g.counter === counter)) return;
    gapNoticesByContact[contactId] = [...list, { counter, timestamp: Date.now() }];
  }

  function gapNoticesFor(contactId: string): { counter: number; timestamp: number }[] {
    return gapNoticesByContact[contactId] ?? [];
  }

  function clearAll() {
    cancelTimers();
    Object.keys(map).forEach((k) => delete map[k]);
    Object.keys(lastTs).forEach((k) => delete lastTs[k]);
    Object.keys(unreadByContact).forEach((k) => delete unreadByContact[k]);
    Object.keys(unreadCapped).forEach((k) => delete unreadCapped[k]);
    Object.keys(firstUnreadByContact).forEach((k) => delete firstUnreadByContact[k]);
    markedUnread = new Set();
    Object.keys(reactions).forEach((k) => delete reactions[k]);
    Object.keys(pinnedByContact).forEach((k) => delete pinnedByContact[k]);
    Object.keys(transferProgress).forEach((k) => delete transferProgress[k]);
    Object.keys(mediaVersions).forEach((k) => delete mediaVersions[k]);
    pendingQueued.clear();
    viaOffline.clear();
    Object.keys(gapNoticesByContact).forEach((k) => delete gapNoticesByContact[k]);
    loadedContacts = new Set();
    oldestReached.clear();
    newestReached.clear();
    pendingSync = new Set();
    pendingSyncDelete = new Set();
    delayedUnseen = {};
  }

  // Seeds unread, delayed-unseen and pending-sync from the core on startup.
  async function hydrate() {
    const [unread, delayed, pending] = await Promise.allSettled([
      getUnreadSummary(),
      getDelayedUnseen(),
      getPendingSync(),
    ]);

    if (unread.status === 'fulfilled') {
      for (const entry of unread.value) {
        unreadByContact[entry.contactId] = entry.count;
        unreadCapped[entry.contactId] = entry.capped;
        if (entry.firstUnread) firstUnreadByContact[entry.contactId] = entry.firstUnread;
        if (entry.markedUnread) markedUnread = new Set(markedUnread).add(entry.contactId);
      }
    } else {
      log.error('Failed to load unread summary', unread.reason);
    }

    if (delayed.status === 'fulfilled') delayedUnseen = delayed.value;
    else log.error('Failed to load delayed-unseen', delayed.reason);

    if (pending.status === 'fulfilled') {
      pendingSync = new Set(pending.value.sync);
      pendingSyncDelete = new Set(pending.value.deleted);
    } else {
      log.error('Failed to load pending-sync', pending.reason);
    }
  }

  return {
    hydrate,
    forContact,
    lastTimestampFor,
    loadFor,
    loadOlder,
    loadNewer,
    loadAround,
    isOldestReached,
    isNewestReached,
    append,
    appendOptimistic,
    queuedFor,
    pendingUploadFor,
    inMailboxFor,
    queuedCount,
    updateStatus,
    updateStatusIfSending,
    markMessagesRead,
    markBundlePublished,
    markFailed,
    retryMessage,
    delayedUnseenFor,
    markDelayedSeen,
    replaceId,
    updateContent,
    markDeleted: removeMessage,
    setAutodownloadPath,
    removeLocally: removeMessage,
    unreadFor,
    unreadCappedFor,
    totalUnread,
    markRead,
    markAllRead,
    markUnread,
    markUnreadFrom,
    isMarkedUnread,
    clearMarkedUnread,
    enterChat,
    leaveChat,
    firstUnreadFor,
    setFirstUnread,
    clearFirstUnread,
    addReaction,
    removeReaction,
    reactionsFor,
    loadPinned,
    pinnedFor,
    applyPinned,
    setPinnedOptimistic,
    setPendingDelete,
    markMediaReady,
    mediaVersionFor,
    markPendingSync,
    isPendingSync,
    isPendingSyncDelete,
    pendingSyncCountFor,
    flushPendingSync,
    setTransferProgress,
    transferProgressFor,
    clearTransferProgress,
    get hasActiveTransfers() {
      return hasActiveTransfers();
    },
    addGapNotice,
    gapNoticesFor,
    clearForContact,
    clearAll,
  };
}

export const messagesState = createMessagesState();
