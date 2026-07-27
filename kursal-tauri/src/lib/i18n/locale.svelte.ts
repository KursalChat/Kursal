import { browser } from '$app/environment';
import en from '../../../../locales/en.json';
import fr from '../../../../locales/fr.json';
import { LOCALES, type Locale } from './locales';

const LOCALE_KEY = 'kursal_locale';
const dicts: Record<Locale, unknown> = { en, fr };

type Vars = Record<string, string | number>;

function isLocale(value: unknown): value is Locale {
  return LOCALES.some((l) => l.id === value);
}

function detect(): Locale {
  if (!browser) return 'en';

  const stored = localStorage.getItem(LOCALE_KEY);
  if (isLocale(stored)) return stored;

  const tag = (navigator.language ?? 'en').toLowerCase().split('-')[0];
  return isLocale(tag) ? tag : 'en';
}

let current = $state<Locale>(detect());

export const locale = {
  get current() {
    return current;
  },
  set(value: Locale) {
    current = value;
    if (browser) localStorage.setItem(LOCALE_KEY, value);
  },
};

function lookup(obj: unknown, path: string): string | undefined {
  const parts = path.split('.');
  let cur = obj;
  for (const part of parts) {
    if (cur == null || typeof cur !== 'object') return undefined;
    cur = (cur as Record<string, unknown>)[part];
  }
  return typeof cur === 'string' ? cur : undefined;
}

export function t(key: string, vars?: Vars): string {
  const raw = lookup(dicts[current], key) ?? lookup(dicts.en, key) ?? key;
  if (!vars) return raw;
  return raw.replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? `{${k}}`));
}

export function tEn(key: string): string {
  return lookup(dicts.en, key) ?? key;
}

// BCP47 tag for Intl/toLocale* formatting, so dates and weekday/month names
// follow the app language instead of the system region.
export function dateLocale(): Locale {
  return current;
}
