import type { ContactResponse, ConnectionChangedPayload } from '$lib/types';
import { log } from '$lib/utils/log';
import { getContacts, getContactMeta, setContactMuted, setContactAlias } from '$lib/api/contacts';
import { bytesToBase64 } from '$lib/utils/base64';
import { shouldStampLastSeen } from '$lib/utils/presence';

function createContactsState() {
  let contacts = $state<ContactResponse[]>([]);
  let loading = $state(false);
  let connectionStatus = $state<Record<string, ConnectionChangedPayload['status']>>({});
  let muted = $state<Record<string, boolean>>({});
  let lastSeen = $state<Record<string, number>>({});
  let aliases = $state<Record<string, string>>({});
  let terminated = $state<Record<string, boolean>>({});

  let markLoaded!: () => void;
  const loaded = new Promise<void>((resolve) => {
    markLoaded = resolve;
  });

  function applyAlias(c: ContactResponse): ContactResponse {
    if (c.profileName === undefined) c.profileName = c.displayName;
    c.displayName = aliases[c.userId] ?? c.profileName;
    return c;
  }

  async function load() {
    loading = true;
    try {
      const result = await getContacts();
      contacts = result.map((c) => {
        if (c.avatarBytes && !c.avatarBase64) {
          c.avatarBase64 = bytesToBase64(c.avatarBytes);
        }
        return c;
      });
    } catch (e) {
      log.error('Failed to load contacts:', e);
    } finally {
      loading = false;
    }
    try {
      const meta = await getContactMeta();
      const m: Record<string, boolean> = {};
      const seen: Record<string, number> = {};
      const al: Record<string, string> = {};
      const term: Record<string, boolean> = {};
      for (const entry of meta) {
        if (entry.muted) m[entry.contactId] = true;
        if (entry.lastSeenAt) seen[entry.contactId] = entry.lastSeenAt;
        if (entry.alias) al[entry.contactId] = entry.alias;
        if (entry.terminated) term[entry.contactId] = true;
      }
      muted = m;
      lastSeen = seen;
      aliases = al;
      terminated = term;
      contacts = contacts.map(applyAlias);
    } catch (e) {
      log.error('Failed to load contact meta:', e);
    }
    markLoaded();
  }

  async function setAlias(contactId: string, alias: string | null) {
    const clean = alias?.trim() || null;
    await setContactAlias(contactId, clean);
    if (clean) aliases[contactId] = clean;
    else delete aliases[contactId];
    const c = contacts.find((c) => c.userId === contactId);
    if (c) {
      applyAlias(c);
      contacts = [...contacts];
    }
  }

  function aliasFor(contactId: string): string | null {
    return aliases[contactId] ?? null;
  }

  function isMuted(contactId: string): boolean {
    return !!muted[contactId];
  }

  function isTerminated(contactId: string): boolean {
    return !!terminated[contactId];
  }

  function setTerminated(contactId: string, value: boolean) {
    if (value) terminated[contactId] = true;
    else delete terminated[contactId];
  }

  async function setMuted(contactId: string, value: boolean) {
    const prev = !!muted[contactId];
    muted[contactId] = value;
    try {
      await setContactMuted(contactId, value);
    } catch (e) {
      muted[contactId] = prev;
      log.error('Failed to set mute:', e);
      throw e;
    }
  }

  function lastSeenAt(contactId: string): number | null {
    return lastSeen[contactId] ?? null;
  }

  function touchLastSeen(contactId: string) {
    lastSeen[contactId] = Date.now();
  }

  function upsert(contact: ContactResponse) {
    if (contact.avatarBytes && !contact.avatarBase64) {
      contact.avatarBase64 = bytesToBase64(contact.avatarBytes);
    }
    applyAlias(contact);

    const idx = contacts.findIndex((c) => c.userId === contact.userId);
    if (idx >= 0) contacts[idx] = contact;
    else contacts.push(contact);
  }

  function remove(contactId: string) {
    const idx = contacts.findIndex((c) => c.userId === contactId);
    if (idx >= 0) contacts.splice(idx, 1);
    delete connectionStatus[contactId];
    delete muted[contactId];
    delete lastSeen[contactId];
    delete aliases[contactId];
    delete terminated[contactId];
  }

  function markVerified(contactId: string) {
    const c = contacts.find((c) => c.userId === contactId);
    if (c) c.verified = true;
  }

  function setConnectionStatus(contactId: string, status: ConnectionChangedPayload['status']) {
    if (shouldStampLastSeen(connectionStatus[contactId], status)) {
      lastSeen[contactId] = Date.now();
    }
    connectionStatus[contactId] = status;
  }

  function getById(id: string) {
    return contacts.find((c) => c.userId === id);
  }

  return {
    get contacts() {
      return contacts;
    },
    get loading() {
      return loading;
    },
    get connectionStatus() {
      return connectionStatus;
    },
    loaded,
    load,
    upsert,
    remove,
    markVerified,
    setConnectionStatus,
    getById,
    isMuted,
    setMuted,
    isTerminated,
    setTerminated,
    lastSeenAt,
    touchLastSeen,
    aliasFor,
    setAlias,
  };
}

export const contactsState = createContactsState();
