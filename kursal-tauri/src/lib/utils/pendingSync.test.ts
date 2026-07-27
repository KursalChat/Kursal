import { describe, expect, it } from 'vitest';
import {
  flushContact,
  parsePendingSync,
  pendingSyncKey,
  serializePendingSync,
  type PendingSyncState,
} from './pendingSync';

const state = (sync: string[], deleted: string[] = []): PendingSyncState => ({
  sync: new Set(sync),
  deleted: new Set(deleted),
});

describe('parsePendingSync', () => {
  it('returns empty state for missing or malformed input', () => {
    for (const raw of [null, '', 'not json', '[]', 'null', '"str"']) {
      const parsed = parsePendingSync(raw);
      expect(parsed.sync.size).toBe(0);
      expect(parsed.deleted.size).toBe(0);
    }
  });

  it('ignores non-string entries rather than trusting stored data', () => {
    const parsed = parsePendingSync(JSON.stringify({ sync: ['a:1', 42, null], deleted: 'nope' }));
    expect([...parsed.sync]).toEqual(['a:1']);
    expect(parsed.deleted.size).toBe(0);
  });
});

describe('serialize/parse round-trip', () => {
  it('survives a restart with markers pending', () => {
    const before = state(['alice:1', 'bob:2'], ['bob:2']);
    const after = parsePendingSync(serializePendingSync(before));
    expect([...after.sync].sort()).toEqual(['alice:1', 'bob:2']);
    expect([...after.deleted]).toEqual(['bob:2']);
  });

  it('round-trips an empty state', () => {
    const after = parsePendingSync(serializePendingSync(state([])));
    expect(after.sync.size).toBe(0);
    expect(after.deleted.size).toBe(0);
  });
});

describe('flushContact', () => {
  it('clears only the named contact', () => {
    const { state: next, changed } = flushContact(state(['alice:1', 'bob:2']), 'alice');
    expect(changed).toBe(true);
    expect([...next.sync]).toEqual(['bob:2']);
  });

  it('reports tombstoned deletes as finalized', () => {
    const { finalizedDeletes, state: next } = flushContact(
      state(['alice:1', 'alice:2'], ['alice:2']),
      'alice'
    );
    expect(finalizedDeletes).toEqual(['2']);
    expect(next.deleted.size).toBe(0);
  });

  it('does not finalize deletes belonging to another contact', () => {
    const { finalizedDeletes, state: next } = flushContact(
      state(['alice:1', 'bob:2'], ['bob:2']),
      'alice'
    );
    expect(finalizedDeletes).toEqual([]);
    expect([...next.deleted]).toEqual(['bob:2']);
  });

  it('reports no change when the contact has no markers', () => {
    const { changed } = flushContact(state(['alice:1']), 'carol');
    expect(changed).toBe(false);
  });

  it('does not treat a contact id as a prefix of a longer one', () => {
    const { state: next } = flushContact(state(['alice:1', 'alice2:1']), 'alice');
    expect([...next.sync]).toEqual(['alice2:1']);
  });

  it('leaves the input state untouched', () => {
    const original = state(['alice:1'], ['alice:1']);
    flushContact(original, 'alice');
    expect(original.sync.size).toBe(1);
    expect(original.deleted.size).toBe(1);
  });
});

describe('pendingSyncKey', () => {
  it('composes contact and message ids', () => {
    expect(pendingSyncKey('alice', '1')).toBe('alice:1');
  });
});
