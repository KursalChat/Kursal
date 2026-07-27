import { browser } from '$app/environment';
import { getUiState, setUiState } from '$lib/api/settings';

const DB_KEY = 'archived_convos';

function parseIds(raw: string | null): string[] {
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((v) => typeof v === 'string') : [];
  } catch {
    return [];
  }
}

function createArchivedConvosState() {
  let archived = $state<Set<string>>(new Set());

  async function init() {
    if (!browser) return;
    const fromDb = parseIds(await getUiState(DB_KEY).catch(() => null));
    archived = new Set([...fromDb, ...archived]);
  }

  async function persist() {
    await setUiState(DB_KEY, archived.size > 0 ? JSON.stringify([...archived]) : '');
  }

  function has(userId: string): boolean {
    return archived.has(userId);
  }

  function toggle(userId: string) {
    const next = new Set(archived);
    if (next.has(userId)) next.delete(userId);
    else next.add(userId);
    archived = next;
    void persist().catch(() => {});
  }

  function remove(userId: string) {
    if (!archived.has(userId)) return;
    const next = new Set(archived);
    next.delete(userId);
    archived = next;
    void persist().catch(() => {});
  }

  return {
    get size() {
      return archived.size;
    },
    init,
    has,
    toggle,
    remove,
  };
}

export const archivedConvosState = createArchivedConvosState();
