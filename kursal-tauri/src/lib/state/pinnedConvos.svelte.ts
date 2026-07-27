import { browser } from '$app/environment';
import { getUiState, setUiState } from '$lib/api/settings';

const DB_KEY = 'pinned_convos';

function parseIds(raw: string | null): string[] {
  if (!raw) return [];
  try {
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed.filter((v) => typeof v === 'string') : [];
  } catch {
    return [];
  }
}

function createPinnedConvosState() {
  let pinned = $state<Set<string>>(new Set());

  async function init() {
    if (!browser) return;
    const fromDb = parseIds(await getUiState(DB_KEY).catch(() => null));
    pinned = new Set([...fromDb, ...pinned]);
  }

  async function persist() {
    await setUiState(DB_KEY, pinned.size > 0 ? JSON.stringify([...pinned]) : '');
  }

  function has(userId: string): boolean {
    return pinned.has(userId);
  }

  function toggle(userId: string) {
    const next = new Set(pinned);
    if (next.has(userId)) next.delete(userId);
    else next.add(userId);
    pinned = next;
    void persist().catch(() => {});
  }

  return { init, has, toggle };
}

export const pinnedConvosState = createPinnedConvosState();
