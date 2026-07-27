import { t } from '$lib/i18n';
import { log } from '$lib/utils/log';
import { notifications } from '$lib/state/notifications.svelte';

export interface AppError {
  code: string;
  message: string;
}

const KNOWN_CODES = new Set([
  'errors.storage',
  'errors.crypto',
  'errors.network',
  'errors.identity',
  'errors.io',
  'errors.unknown',
]);

export function parseError(e: unknown): AppError {
  if (e && typeof e === 'object' && 'code' in e && 'message' in e) {
    const code = String((e as AppError).code);
    return { code, message: String((e as AppError).message) };
  }
  if (typeof e === 'string') return { code: 'unknown', message: e };
  if (e instanceof Error) return { code: 'unknown', message: e.message };
  return { code: 'unknown', message: String(e) };
}

// Resolves a localized, user-safe message. A provided fallbackKey (action
// context, e.g. "settings.account.errorExport") wins when it exists; otherwise
// falls back to a per-category message keyed by the backend error code.
export function errorText(e: unknown, fallbackKey?: string): string {
  if (fallbackKey) {
    const resolved = t(fallbackKey);
    if (resolved && resolved !== fallbackKey) return resolved;
  }
  const { code } = parseError(e);
  return t(KNOWN_CODES.has(code) ? code : 'errors.unknown');
}

// Logs the raw technical detail and shows a localized toast. Replaces the
// log.error + notifications.push(String(e)) boilerplate.
export function notifyError(e: unknown, fallbackKey?: string): void {
  log.error(parseError(e).message, e);
  notifications.push(errorText(e, fallbackKey), 'error');
}
