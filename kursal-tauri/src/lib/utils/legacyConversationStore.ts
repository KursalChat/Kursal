// TODO: remove once migration done
const LEGACY_KEYS = [
  'kursal:unread',
  'kursal:delayedUnseen',
  'kursal:pendingSync',
  'kursal:autodownloadPaths',
];

export interface LegacyConversationState {
  // contactId -> id of the first unread message, when it was known.
  firstUnread: Record<string, string>;
  // Contacts with unread messages but no recorded anchor.
  unreadOnly: string[];
  delayedUnseen: Record<string, string[]>;
}

function parseObject(raw: string | null): Record<string, unknown> | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === 'object' && !Array.isArray(parsed) ? parsed : null;
  } catch {
    return null;
  }
}

function stringRecord(value: unknown): Record<string, string> {
  const out: Record<string, string> = {};
  if (!value || typeof value !== 'object') return out;
  for (const [k, v] of Object.entries(value)) if (typeof v === 'string') out[k] = v;
  return out;
}

function stringListRecord(value: unknown): Record<string, string[]> {
  const out: Record<string, string[]> = {};
  if (!value || typeof value !== 'object') return out;
  for (const [k, v] of Object.entries(value)) {
    if (!Array.isArray(v)) continue;
    const ids = v.filter((id): id is string => typeof id === 'string');
    if (ids.length) out[k] = ids;
  }
  return out;
}

function countRecord(value: unknown): Record<string, number> {
  const out: Record<string, number> = {};
  if (!value || typeof value !== 'object') return out;
  for (const [k, v] of Object.entries(value)) if (typeof v === 'number' && v > 0) out[k] = v;
  return out;
}

export function readLegacyConversationState(): LegacyConversationState | null {
  if (typeof localStorage === 'undefined') return null;
  if (!LEGACY_KEYS.some((key) => localStorage.getItem(key) !== null)) return null;

  const unread = parseObject(localStorage.getItem('kursal:unread'));
  const counts = countRecord(unread?.counts);

  const firstUnread: Record<string, string> = {};
  for (const [contactId, id] of Object.entries(stringRecord(unread?.first))) {
    if (counts[contactId]) firstUnread[contactId] = id;
  }

  const unreadOnly = Object.keys(counts).filter((contactId) => !firstUnread[contactId]);
  const delayedUnseen = stringListRecord(parseObject(localStorage.getItem('kursal:delayedUnseen')));

  return { firstUnread, unreadOnly, delayedUnseen };
}

export function clearLegacyConversationState() {
  if (typeof localStorage === 'undefined') return;
  for (const key of LEGACY_KEYS) localStorage.removeItem(key);
}
