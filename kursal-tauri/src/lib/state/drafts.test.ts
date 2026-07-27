import { describe, it, expect } from 'vitest';
import { draftsState } from './drafts.svelte';

describe('draftsState', () => {
  it('returns empty string for an unknown contact', () => {
    expect(draftsState.get('unknown-contact')).toBe('');
  });

  it('stores and retrieves a draft per contact', () => {
    draftsState.set('c1', 'hello');
    expect(draftsState.get('c1')).toBe('hello');
  });

  it('deletes the draft when set to empty text', () => {
    draftsState.set('c2', 'draft');
    draftsState.set('c2', '');
    expect(draftsState.get('c2')).toBe('');
  });

  it('clear() removes a draft', () => {
    draftsState.set('c3', 'x');
    draftsState.clear('c3');
    expect(draftsState.get('c3')).toBe('');
  });

  it('keeps drafts isolated between contacts', () => {
    draftsState.set('a', 'draft-a');
    draftsState.set('b', 'draft-b');
    expect(draftsState.get('a')).toBe('draft-a');
    expect(draftsState.get('b')).toBe('draft-b');
  });
});
