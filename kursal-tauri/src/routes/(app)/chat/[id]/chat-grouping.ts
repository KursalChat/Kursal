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
  return [...list].sort(
    (a, b) => offlineTier(a.status, peerOnline) - offlineTier(b.status, peerOnline)
  );
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
