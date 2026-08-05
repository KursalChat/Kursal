// Tracks the core's periodic offline-mailbox poll (every 15 min, skipped while
// no peers are connected). Fed by the `offline_sync` Tauri event.
function createOfflineSyncState() {
  let active = $state(false);
  let lastCompletedAt = $state<number | null>(null);

  function setActive(value: boolean) {
    // A poll that never reported a start still counts as completed: the core
    // can emit only the trailing edge if the UI mounted mid-poll.
    if (!value) lastCompletedAt = Date.now();
    active = value;
  }

  return {
    get active() {
      return active;
    },
    get lastCompletedAt() {
      return lastCompletedAt;
    },
    setActive,
  };
}

export const offlineSyncState = createOfflineSyncState();
