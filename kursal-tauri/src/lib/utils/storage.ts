import { browser } from '$app/environment';

export function readRaw(key: string): string | null {
  if (!browser) return null;
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

// Writes are best-effort: quota exhaustion or a disabled store must not break a
// user action that only wanted to remember a preference.
export function writeRaw(key: string, value: string): void {
  if (!browser) return;
  try {
    localStorage.setItem(key, value);
  } catch {}
}

export function removeRaw(key: string): void {
  if (!browser) return;
  try {
    localStorage.removeItem(key);
  } catch {}
}

export function readJson<T>(key: string, fallback: T): T {
  const raw = readRaw(key);
  if (!raw) return fallback;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

export function writeJson<T>(key: string, value: T): void {
  writeRaw(key, JSON.stringify(value));
}

export function readStringSet(key: string): Set<string> {
  const parsed = readJson<unknown>(key, null);
  if (!Array.isArray(parsed)) return new Set();
  return new Set(parsed.filter((x): x is string => typeof x === 'string'));
}

export function writeStringSet(key: string, set: Set<string>): void {
  writeJson(key, [...set]);
}

export function readFlag(key: string): boolean {
  return readRaw(key) === '1';
}

export function writeFlag(key: string, value: boolean): void {
  if (value) writeRaw(key, '1');
  else removeRaw(key);
}
