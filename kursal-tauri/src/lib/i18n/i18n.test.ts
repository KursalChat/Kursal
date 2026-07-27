import { describe, it, expect } from 'vitest';
import { t } from './locale.svelte';

describe('t()', () => {
  it('looks up a nested key', () => {
    expect(t('common.cancel')).toBe('Cancel');
  });

  it('interpolates variables', () => {
    expect(t('chat.composer.placeholder', { name: 'Bob' })).toBe('Message Bob');
  });

  it('returns the key itself when missing', () => {
    expect(t('does.not.exist')).toBe('does.not.exist');
  });

  it('leaves literal text untouched when vars are irrelevant', () => {
    expect(t('common.cancel', { unused: 'x' })).toBe('Cancel');
  });

  it('keeps placeholders with no matching var', () => {
    expect(t('chat.composer.placeholder')).toBe('Message {name}');
  });
});
