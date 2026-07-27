function createTypingState() {
  let typingByContact = $state<Record<string, { replyTo: string | null }>>({});
  const timers = new Map<string, ReturnType<typeof setTimeout>>();

  function isTyping(contactId: string): boolean {
    return !!typingByContact[contactId];
  }

  function replyToFor(contactId: string): string | null {
    return typingByContact[contactId]?.replyTo ?? null;
  }

  function set(contactId: string, replyTo: string | null) {
    typingByContact[contactId] = { replyTo };
    const existing = timers.get(contactId);
    if (existing) clearTimeout(existing);
    const t = setTimeout(() => {
      clear(contactId);
    }, 10_000);
    timers.set(contactId, t);
  }

  function clear(contactId: string) {
    if (typingByContact[contactId]) {
      const next = { ...typingByContact };
      delete next[contactId];
      typingByContact = next;
    }
    const t = timers.get(contactId);
    if (t) {
      clearTimeout(t);
      timers.delete(contactId);
    }
  }

  return { isTyping, replyToFor, set, clear };
}

export const typingState = createTypingState();
