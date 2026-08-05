import { discardPendingShare, takePendingShares } from '$lib/api/share';
import { log } from '$lib/utils/log';
import type { SharePayload } from '$lib/types';

declare global {
  interface Window {
    __kursalOnShare?: () => void;
  }
}

/**
 * Payloads handed over by the OS share sheet, staged on disk by native code.
 * `drain()` loads them, `assign()` records the picked contact, `claim()` hands
 * the payload to the mounted chat page; `release()`/`discardHead()` clean up.
 */
function createShareIntentState() {
  let queue = $state<SharePayload[]>([]);
  let target = $state<string | null>(null);
  let draining = false;

  async function release(id: string) {
    try {
      await discardPendingShare(id);
    } catch (e) {
      log.error('Failed to discard staged share:', e);
    }
  }

  async function drain() {
    if (draining) return;
    draining = true;
    try {
      const payloads = await takePendingShares();
      if (payloads.length) queue = [...queue, ...payloads];
    } catch (e) {
      log.error('Failed to drain pending shares:', e);
    } finally {
      draining = false;
    }
  }

  return {
    get head() {
      return queue[0] ?? null;
    },
    /** A payload is waiting and no contact has been picked for it yet. */
    get awaitingTarget() {
      return queue.length > 0 && target === null;
    },

    drain,

    assign(contactId: string) {
      if (queue.length) target = contactId;
    },

    claim(contactId: string): SharePayload | null {
      const head = queue[0];
      if (!head || target !== contactId) return null;
      queue = queue.slice(1);
      target = null;
      return head;
    },

    async discardHead() {
      const head = queue[0];
      if (!head) return;
      queue = queue.slice(1);
      target = null;
      await release(head.id);
    },

    release,

    /**
     * MainActivity calls `window.__kursalOnShare()` once a staged payload is
     * complete on disk, which can land after the startup drain when the copy is
     * slow. Every other platform never calls it.
     */
    listenForNative(): () => void {
      if (typeof window === 'undefined') return () => {};
      window.__kursalOnShare = () => void drain();
      return () => {
        delete window.__kursalOnShare;
      };
    },

    reset() {
      queue = [];
      target = null;
    },
  };
}

export const shareIntentState = createShareIntentState();
