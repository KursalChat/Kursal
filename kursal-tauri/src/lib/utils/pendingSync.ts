export interface PendingSyncState {
  sync: Set<string>;
  deleted: Set<string>;
}

export function pendingSyncKey(contactId: string, messageId: string): string {
  return `${contactId}:${messageId}`;
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
