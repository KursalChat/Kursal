import { readStringSet, writeStringSet } from '$lib/utils/storage';

const STORAGE_KEY = 'kursal:trustedDomains';

const BUILTIN_TRUSTED = new Set<string>(['kursal.chat']);

function normalize(host: string): string {
  return host
    .trim()
    .toLowerCase()
    .replace(/^\.+|\.+$/g, '');
}

function createTrustedDomainsState() {
  let domains = $state<string[]>([...readStringSet(STORAGE_KEY)].sort());

  function isTrusted(host: string): boolean {
    const h = normalize(host);
    if (!h) return false;
    if (BUILTIN_TRUSTED.has(h)) return true;
    return domains.includes(h);
  }

  function trust(host: string) {
    const h = normalize(host);
    if (!h || BUILTIN_TRUSTED.has(h) || domains.includes(h)) return;
    const next = [...domains, h].sort();
    domains = next;
    writeStringSet(STORAGE_KEY, new Set(next));
  }

  function untrust(host: string) {
    const h = normalize(host);
    if (!domains.includes(h)) return;
    const next = domains.filter((d) => d !== h);
    domains = next;
    writeStringSet(STORAGE_KEY, new Set(next));
  }

  function clear() {
    domains = [];
    writeStringSet(STORAGE_KEY, new Set());
  }

  return {
    get domains() {
      return domains;
    },
    get builtin() {
      return [...BUILTIN_TRUSTED];
    },
    isTrusted,
    trust,
    untrust,
    clear,
  };
}

export const trustedDomainsState = createTrustedDomainsState();
