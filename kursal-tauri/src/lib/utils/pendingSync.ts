// "Waiting to sync" markers for edits/reactions/deletes the backend put on a
// contact's offline queue. They must outlive a restart: the backend's queue is
// persisted, so markers kept only in memory would disappear while the queued
// work is still pending.
//
// Entries are `contactId:messageId` composite keys and are cleared per-contact
// when the backend reports that contact's queue drained.
export const PENDING_SYNC_STORAGE_KEY = 'kursal:pendingSync';

export interface PendingSyncState {
  sync: Set<string>;
  deleted: Set<string>;
}

export function pendingSyncKey(contactId: string, messageId: string): string {
  return `${contactId}:${messageId}`;
}

function toStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((v): v is string => typeof v === 'string') : [];
}

export function parsePendingSync(raw: string | null): PendingSyncState {
  const empty: PendingSyncState = { sync: new Set(), deleted: new Set() };
  if (!raw) return empty;
  try {
    const parsed = JSON.parse(raw);
    if (!parsed || typeof parsed !== 'object') return empty;
    return {
      sync: new Set(toStringArray(parsed.sync)),
      deleted: new Set(toStringArray(parsed.deleted)),
    };
  } catch {
    return empty;
  }
}

export function serializePendingSync(state: PendingSyncState): string {
  return JSON.stringify({
    sync: [...state.sync],
    deleted: [...state.deleted],
  });
}

export interface PendingSyncFlush {
  state: PendingSyncState;
  /** messageIds whose tombstoned delete is now final and can be dropped from the UI */
  finalizedDeletes: string[];
  changed: boolean;
}

/**
 * Drops every marker belonging to `contactId`, reporting which tombstoned
 * deletes became final so the caller can remove those messages.
 */
export function flushContact(state: PendingSyncState, contactId: string): PendingSyncFlush {
  const prefix = `${contactId}:`;
  const sync = new Set(state.sync);
  const deleted = new Set(state.deleted);
  const finalizedDeletes: string[] = [];
  let changed = false;

  for (const key of state.sync) {
    if (!key.startsWith(prefix)) continue;
    sync.delete(key);
    changed = true;
    if (deleted.has(key)) {
      deleted.delete(key);
      finalizedDeletes.push(key.slice(prefix.length));
    }
  }

  return { state: { sync, deleted }, finalizedDeletes, changed };
}
