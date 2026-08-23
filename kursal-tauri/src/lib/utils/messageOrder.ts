import type { MessageResponse } from '$lib/types';

// Inserts msg into list keeping ascending (timestamp, id) order. Returns the
// index it landed at, so callers can tell whether it appended at the tail.
export function insertInSentOrder(list: MessageResponse[], msg: MessageResponse): number {
  let lo = 0;
  let hi = list.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    const c = list[mid];
    const before = c.timestamp < msg.timestamp || (c.timestamp === msg.timestamp && c.id < msg.id);
    if (before) lo = mid + 1;
    else hi = mid;
  }
  list.splice(lo, 0, msg);
  return lo;
}
