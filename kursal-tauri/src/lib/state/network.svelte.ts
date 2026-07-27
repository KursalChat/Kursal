function createNetworkState() {
  let online = $state(true);
  let peerCount = $state(0);
  let initialized = $state(false);

  function set(next: boolean, count: number) {
    online = next;
    peerCount = count;
    initialized = true;
  }

  return {
    get online() {
      return online;
    },
    get peerCount() {
      return peerCount;
    },
    get initialized() {
      return initialized;
    },
    set,
  };
}

export const networkState = createNetworkState();
