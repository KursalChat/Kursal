import { browser } from '$app/environment';
import { getUiState, setUiState } from '$lib/api/settings';

function parseIds(raw: string | null): string[] {
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((v) => typeof v === 'string') : [];
  } catch {
    return [];
  }
}

export function createIdSetState(dbKey: string) {
  let ids = $state<Set<string>>(new Set());

  async function init() {
    if (!browser) return;
    const fromDb = parseIds(await getUiState(dbKey).catch(() => null));
    ids = new Set([...fromDb, ...ids]);
  }

  async function persist() {
    await setUiState(dbKey, ids.size > 0 ? JSON.stringify([...ids]) : '');
  }

  function commit(next: Set<string>) {
    ids = next;
    void persist().catch(() => {});
  }

  return {
    get size() {
      return ids.size;
    },
    init,
    has(userId: string): boolean {
      return ids.has(userId);
    },
    toggle(userId: string) {
      const next = new Set(ids);
      if (next.has(userId)) next.delete(userId);
      else next.add(userId);
      commit(next);
    },
    remove(userId: string) {
      if (!ids.has(userId)) return;
      const next = new Set(ids);
      next.delete(userId);
      commit(next);
    },
  };
}
