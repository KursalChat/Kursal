import { browser } from '$app/environment';
import { getUiState, setUiState } from '$lib/api/settings';

const DB_KEY = 'session.lastContact';

function createSessionState() {
  let lastContactId = $state<string | null>(null);

  // Returns what was on disk so callers act on that rather than on the live
  // value, which a chat opened during the read may already have replaced.
  async function init(): Promise<string | null> {
    if (!browser) return null;
    const stored = await getUiState(DB_KEY).catch(() => null);
    if (stored && lastContactId === null) lastContactId = stored;
    return stored || null;
  }

  function setLastContact(contactId: string) {
    if (!browser || contactId === lastContactId) return;
    lastContactId = contactId;
    void setUiState(DB_KEY, contactId).catch(() => {});
  }

  function clearLastContact() {
    if (!browser) return;
    lastContactId = null;
    void setUiState(DB_KEY, '').catch(() => {});
  }

  return {
    get lastContactId() {
      return lastContactId;
    },
    init,
    setLastContact,
    clearLastContact,
  };
}

export const sessionState = createSessionState();
