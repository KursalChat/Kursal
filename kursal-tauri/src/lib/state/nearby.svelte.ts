import type { NearbyPeerResponse } from '$lib/types';

function createNearbyState() {
  let peers = $state<NearbyPeerResponse[]>([]);
  // For incoming ConnectRequest decisions.
  let pendingRequests = $state<Record<string, string>>({});
  let active = $state(false);
  let mySessionName = $state<string | null>(null);

  function setPeers(newPeers: NearbyPeerResponse[]) {
    peers = newPeers;
  }

  function addPendingRequest(peerId: string, sessionName: string) {
    pendingRequests = {
      ...pendingRequests,
      [peerId]: sessionName,
    };
  }

  function removePendingRequest(peerId: string) {
    const { [peerId]: _removed, ...rest } = pendingRequests;
    pendingRequests = rest;
  }

  function reset() {
    peers = [];
    pendingRequests = {};
    active = false;
    mySessionName = null;
  }

  return {
    get peers() {
      return peers;
    },
    get pendingRequests() {
      return pendingRequests;
    },
    get active() {
      return active;
    },
    get mySessionName() {
      return mySessionName;
    },
    set active(value: boolean) {
      active = value;
    },
    set mySessionName(value: string | null) {
      mySessionName = value;
    },
    setPeers,
    addPendingRequest,
    removePendingRequest,
    reset,
  };
}

export const nearbyState = createNearbyState();
