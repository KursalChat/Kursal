import { browser } from '$app/environment';
import { getUiState, setUiState } from '$lib/api/settings';

const DB_KEY = 'drafts';
const PERSIST_DEBOUNCE_MS = 400;

function parseRecord(raw: string | null): Record<string, string> {
  if (!raw) return {};
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : {};
  } catch {
    return {};
  }
}

function createDraftsState() {
  let drafts = $state<Record<string, string>>({});
  let persistTimer: ReturnType<typeof setTimeout> | null = null;

  async function init() {
    if (!browser) return;
    const fromDb = parseRecord(await getUiState(DB_KEY).catch(() => null));
    drafts = { ...fromDb, ...drafts };
  }

  async function flush() {
    if (persistTimer) {
      clearTimeout(persistTimer);
      persistTimer = null;
    }
    const value = Object.keys(drafts).length > 0 ? JSON.stringify(drafts) : '';
    await setUiState(DB_KEY, value);
  }

  function persist() {
    if (!browser) return;
    if (persistTimer) clearTimeout(persistTimer);
    persistTimer = setTimeout(() => {
      persistTimer = null;
      void flush().catch(() => {});
    }, PERSIST_DEBOUNCE_MS);
  }

  function get(contactId: string): string {
    return drafts[contactId] ?? '';
  }

  function set(contactId: string, text: string) {
    if (text) drafts[contactId] = text;
    else delete drafts[contactId];
    persist();
  }

  function clear(contactId: string) {
    delete drafts[contactId];
    persist();
  }

  return { init, get, set, clear };
}

export const draftsState = createDraftsState();
