// Client-side mirror of `ProfileInfo::validate` in
// kursal-core/src/messaging/enums.rs. Keep both in sync: the backend is the
// authority and will still reject anything that slips past this check.

export const DISPLAY_NAME_MIN = 3;
export const DISPLAY_NAME_MAX = 32;
export const MAX_PROFILE_AVATAR_LEN = 256 * 1000;

// Rust `char::is_alphanumeric()` is Alphabetic || numeric (Nd/Nl/No).
const ALLOWED_CHAR = /[\p{Alphabetic}\p{Nd}\p{Nl}\p{No} ._'-]/u;

export type DisplayNameError =
  'empty' | 'length' | 'edgeWhitespace' | 'unsupportedChars' | 'avatarTooLarge';

/** Returns an i18n suffix key for the first broken rule, or null when valid. */
export function validateDisplayName(name: string): DisplayNameError | null {
  if (name.trim().length === 0) return 'empty';

  const chars = [...name];
  if (chars.length < DISPLAY_NAME_MIN || chars.length > DISPLAY_NAME_MAX) return 'length';

  if (name.trim() !== name) return 'edgeWhitespace';

  if (!chars.every((c) => ALLOWED_CHAR.test(c))) return 'unsupportedChars';

  return null;
}

export function validateAvatarBytes(bytes: number[] | null | undefined): DisplayNameError | null {
  if (bytes && bytes.length > MAX_PROFILE_AVATAR_LEN) return 'avatarTooLarge';
  return null;
}

/**
 * Best-effort cleanup applied while typing: drops unsupported characters and collapses
 * to the maximum length. Leading/trailing spaces are left alone so the field stays
 * usable mid-edit; `validateDisplayName` catches them on submit.
 */
export function sanitizeDisplayNameInput(raw: string): string {
  const kept = [...raw].filter((c) => ALLOWED_CHAR.test(c));
  return kept.slice(0, DISPLAY_NAME_MAX).join('');
}
