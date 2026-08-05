const STORAGE_KEY = 'kursal:trustedDomains';

const BUILTIN_TRUSTED = new Set<string>(['kursal.chat']);

function readSet(): Set<string> {
  if (typeof localStorage === 'undefined') return new Set();
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return new Set();
    const arr = JSON.parse(raw);
    if (!Array.isArray(arr)) return new Set();
    return new Set(arr.filter((x): x is string => typeof x === 'string'));
  } catch {
    return new Set();
  }
}

function writeSet(set: Set<string>) {
  if (typeof localStorage === 'undefined') return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify([...set]));
  } catch {
    /* quota or disabled: silent */
  }
}

function normalize(host: string): string {
  return host
    .trim()
    .toLowerCase()
    .replace(/^\.+|\.+$/g, '');
}

function createTrustedDomainsState() {
  let domains = $state<string[]>([...readSet()].sort());

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
    writeSet(new Set(next));
  }

  function untrust(host: string) {
    const h = normalize(host);
    if (!domains.includes(h)) return;
    const next = domains.filter((d) => d !== h);
    domains = next;
    writeSet(new Set(next));
  }

  function clear() {
    domains = [];
    writeSet(new Set());
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
