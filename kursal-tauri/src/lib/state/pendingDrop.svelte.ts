// Routes OS file drops that land on a sidebar contact row to that contact's
// chat: the (app) layout stores the paths here and navigates; the chat page
// consumes them into its send staging.

export function contactDropTargetAt(pos: { x: number; y: number }): string | null {
  const scale = window.devicePixelRatio || 1;
  const el = document.elementFromPoint(pos.x / scale, pos.y / scale);
  const row = el?.closest?.('[data-contact-drop]') as HTMLElement | null;
  return row?.dataset.contactDrop ?? null;
}

function createPendingDropState() {
  let pending = $state<{ contactId: string; paths: string[] } | null>(null);
  let hoverId = $state<string | null>(null);

  return {
    get pending() {
      return pending;
    },
    get hoverId() {
      return hoverId;
    },
    set(contactId: string, paths: string[]) {
      pending = { contactId, paths };
    },
    setHover(id: string | null) {
      hoverId = id;
    },
    consume(contactId: string): string[] | null {
      if (!pending || pending.contactId !== contactId) return null;
      const paths = pending.paths;
      pending = null;
      return paths;
    },
  };
}

export const pendingDropState = createPendingDropState();
