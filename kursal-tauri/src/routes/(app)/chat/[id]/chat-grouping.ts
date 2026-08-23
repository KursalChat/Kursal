import type { MessageResponse } from '$lib/types';

export interface MessageGroup {
  kind: 'msgs' | 'call' | 'pin';
  direction: 'sent' | 'received';
  messages: MessageResponse[];
  timestamp: number;
  tier: number;
}

const GROUP_MERGE_WINDOW_MS = 60_000;

// While the peer is offline, undelivered sends sink below the chronological messages
// in two tiers: 1 = stored in the DHT, 2 = still waiting to upload. A brief "sending"
// while the peer is reachable is normal direct delivery, so it stays tier 0.
export function offlineTier(status: string, peerOnline: boolean): number {
  if (peerOnline) return 0;
  if (status === 'queued_in_dht') return 1;
  if (status === 'queued' || status === 'sending') return 2;
  return 0;
}

export function sortByOfflineTier(list: MessageResponse[], peerOnline: boolean): MessageResponse[] {
  if (!list.some((m) => offlineTier(m.status, peerOnline) > 0)) return list;
  // Three known buckets, so partitioning beats a comparison sort and keeps
  // each tier in its original order without needing a stable-sort guarantee.
  const tiers: MessageResponse[][] = [[], [], []];
  for (const m of list) tiers[offlineTier(m.status, peerOnline)].push(m);
  return tiers[0].concat(tiers[1], tiers[2]);
}

// Groups consecutive same-direction messages (within the merge window, same tier,
// not split by the unread separator). A connected call emits two records (started +
// terminal); the 'started' one is hidden once its terminal exists, so it renders as one line.
export function buildMessageGroups(
  visible: MessageResponse[],
  peerOnline: boolean,
  firstUnreadId: string | null
): MessageGroup[] {
  const groups: MessageGroup[] = [];
  const calls = visible.filter((m) => m.callDetails);
  const hiddenStarted = new Set<string>();
  for (let i = 0; i < calls.length; i++) {
    const next = calls[i + 1]?.callDetails;
    if (calls[i].callDetails!.outcome === 'started' && next && next.outcome !== 'started') {
      hiddenStarted.add(calls[i].id);
    }
  }
  for (const msg of visible) {
    if (hiddenStarted.has(msg.id)) continue;
    const isCall = !!msg.callDetails;
    const isPin = !!msg.pinDetails;
    const last = groups[groups.length - 1];
    const tier = offlineTier(msg.status, peerOnline);
    const canMerge =
      !isCall &&
      !isPin &&
      msg.id !== firstUnreadId &&
      last &&
      last.kind === 'msgs' &&
      last.direction === msg.direction &&
      last.tier === tier &&
      msg.timestamp - last.messages[last.messages.length - 1].timestamp < GROUP_MERGE_WINDOW_MS;
    if (canMerge) {
      last.messages.push(msg);
    } else {
      groups.push({
        kind: isCall ? 'call' : isPin ? 'pin' : 'msgs',
        direction: msg.direction,
        messages: [msg],
        timestamp: msg.timestamp,
        tier,
      });
    }
  }
  return groups;
}

export interface ImageRun {
  msgs: MessageResponse[];
  startIdx: number;
}

const STACK_MIN = 4;

// Four or more consecutive plain images collapse into one stack tile. Anything
// carrying its own chrome (a reply, a pin, reactions) stays its own bubble.
export function isStackableImage(
  m: MessageResponse,
  isImage: (filename: string) => boolean,
  hasReactions: (m: MessageResponse) => boolean
): boolean {
  if (!m.fileDetails) return false;
  if (!isImage(m.fileDetails.filename)) return false;
  if (m.replyTo || m.pinned) return false;
  if (m.status === 'failed') return false;
  return !hasReactions(m);
}

export function imageRuns(
  msgs: MessageResponse[],
  stackable: (m: MessageResponse) => boolean
): ImageRun[] {
  const runs: ImageRun[] = [];
  let i = 0;
  while (i < msgs.length) {
    if (stackable(msgs[i])) {
      let j = i;
      while (j < msgs.length && stackable(msgs[j])) j++;
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
