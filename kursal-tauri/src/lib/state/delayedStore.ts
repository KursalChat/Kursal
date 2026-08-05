import type { MessageResponse } from '$lib/types';

const KEY = 'kursal:delayedUnseen';

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

export function loadDelayed(): Record<string, string[]> {
  if (typeof localStorage === 'undefined') return {};
  try {
    const raw = localStorage.getItem(KEY);
    const parsed = raw ? JSON.parse(raw) : {};
    return parsed && typeof parsed === 'object' ? parsed : {};
  } catch {
    return {};
  }
}

export function saveDelayed(store: Record<string, string[]>) {
  if (typeof localStorage === 'undefined') return;
  try {
    localStorage.setItem(KEY, JSON.stringify(store));
  } catch {
    // Non-fatal: quota exceeded or storage unavailable.
  }
}
