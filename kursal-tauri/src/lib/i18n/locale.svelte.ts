import { browser } from '$app/environment';
import en from '../../../../locales/en.json';
import { LOCALES, type Locale } from './locales';

const LOCALE_KEY = 'kursal_locale';

type Vars = Record<string, string | number>;
type Dict = Map<string, string>;

function flatten(obj: unknown, prefix = '', out: Dict = new Map()): Dict {
  if (obj == null || typeof obj !== 'object') return out;
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? `${prefix}.${k}` : k;
    if (typeof v === 'string') out.set(key, v);
    else flatten(v, key, out);
  }
  return out;
}

const enDict = flatten(en);

type Translated = Exclude<Locale, 'en'>;

const loaders: Record<Translated, () => Promise<{ default: unknown }>> = {
  fr: () => import('../../../../locales/fr.json'),
};

const dicts = $state<Partial<Record<Locale, Dict>>>({ en: enDict });

async function loadDict(id: Locale): Promise<void> {
  if (dicts[id]) return;
  const loader = loaders[id as Translated];
  if (!loader) return;
  try {
    dicts[id] = flatten((await loader()).default);
  } catch {
    // Non-fatal: t() keeps falling back to en.
  }
}

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

const initial = detect();
let current = $state<Locale>(initial);

export const localeReady = loadDict(initial);

export const locale = {
  get current() {
    return current;
  },
  set(value: Locale) {
    current = value;
    if (browser) localStorage.setItem(LOCALE_KEY, value);
    void loadDict(value);
  },
};

export function t(key: string, vars?: Vars): string {
  const raw = dicts[current]?.get(key) ?? enDict.get(key) ?? key;
  if (!vars) return raw;
  return raw.replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? `{${k}}`));
}

export function tEn(key: string): string {
  return enDict.get(key) ?? key;
}

// BCP47 tag for Intl/toLocale* formatting, so dates and weekday/month names
// follow the app language instead of the system region.
export function dateLocale(): Locale {
  return current;
}