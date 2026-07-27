import { describe, expect, it } from 'vitest';
import {
  MAX_PROFILE_AVATAR_LEN,
  sanitizeDisplayNameInput,
  validateAvatarBytes,
  validateDisplayName,
} from './displayName';

describe('validateDisplayName', () => {
  it('accepts names the backend accepts', () => {
    expect(validateDisplayName('Kubik')).toBeNull();
    expect(validateDisplayName('a.b_c-d')).toBeNull();
    expect(validateDisplayName("O'Brien")).toBeNull();
    expect(validateDisplayName('Jean Dupont')).toBeNull();
    expect(validateDisplayName('Zoë 123')).toBeNull();
    expect(validateDisplayName('日本語名')).toBeNull();
  });

  it('rejects empty or whitespace-only names', () => {
    expect(validateDisplayName('')).toBe('empty');
    expect(validateDisplayName('   ')).toBe('empty');
  });

  it('enforces the 3..=32 character range by code point', () => {
    expect(validateDisplayName('ab')).toBe('length');
    expect(validateDisplayName('abc')).toBeNull();
    expect(validateDisplayName('a'.repeat(32))).toBeNull();
    expect(validateDisplayName('a'.repeat(33))).toBe('length');
  });

  it('rejects leading and trailing whitespace', () => {
    expect(validateDisplayName(' Kubik')).toBe('edgeWhitespace');
    expect(validateDisplayName('Kubik ')).toBe('edgeWhitespace');
  });

  it('rejects characters outside the allowed set', () => {
    expect(validateDisplayName('Kubik!')).toBe('unsupportedChars');
    expect(validateDisplayName('a@b.com')).toBe('unsupportedChars');
    expect(validateDisplayName('hey\nthere')).toBe('unsupportedChars');
    expect(validateDisplayName('emoji 🎉')).toBe('unsupportedChars');
  });
});

describe('validateAvatarBytes', () => {
  it('accepts missing or in-budget avatars', () => {
    expect(validateAvatarBytes(null)).toBeNull();
    expect(validateAvatarBytes([])).toBeNull();
    expect(validateAvatarBytes(new Array(MAX_PROFILE_AVATAR_LEN).fill(0))).toBeNull();
  });

  it('rejects avatars over the backend limit', () => {
    expect(validateAvatarBytes(new Array(MAX_PROFILE_AVATAR_LEN + 1).fill(0))).toBe(
      'avatarTooLarge'
    );
  });
});

describe('sanitizeDisplayNameInput', () => {
  it('drops unsupported characters', () => {
    expect(sanitizeDisplayNameInput('Ku!b@ik#')).toBe('Kubik');
    expect(sanitizeDisplayNameInput('hey\nthere')).toBe('heythere');
  });

  it('truncates to the maximum length', () => {
    expect(sanitizeDisplayNameInput('a'.repeat(40))).toHaveLength(32);
  });

  it('preserves allowed punctuation and spaces mid-edit', () => {
    expect(sanitizeDisplayNameInput("Jean-Luc O'Neil_1 ")).toBe("Jean-Luc O'Neil_1 ");
  });
});
