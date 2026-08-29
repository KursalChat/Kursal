import { log } from '$lib/utils/log';
import { optimistic } from '$lib/utils/optimistic';
import {
  getPeerRotationInterval,
  setPeerRotationInterval,
  getTypingIndicatorsEnabled,
  setTypingIndicatorsEnabled,
  getReadReceiptsEnabled,
  setReadReceiptsEnabled,
  getRelayConfig,
  setRelayConfig,
  getListeningPort,
  setListeningPort,
  getNearbyShareEnabled,
  setNearbyShareEnabled,
  getAutoAcceptConfig,
  setAutoAcceptConfig,
  getAutoDownloadConfig,
  setAutoDownloadConfig,
  getLocalApiConfig,
  setLocalApiConfig,
  getNodes,
  addCustomNode,
  removeCustomNode,
  type PeerRotationInterval,
  type RelayConfig,
  type AutoAcceptConfig,
  type AutoDownloadConfig,
  type LocalApiConfig,
  type NodesResponse,
} from '$lib/api/settings';

const DEFAULT_RELAY: RelayConfig = {
  maxConnections: 100,
  maxConnectionsPerIp: 10,
};
const DEFAULT_AUTO_ACCEPT: AutoAcceptConfig = {
  mode: 'nobody',
  sizeCapBytes: 2 * 1024 * 1024,
};
const DEFAULT_AUTO_DOWNLOAD: AutoDownloadConfig = {
  scope: 'per_contact',
  limitBytes: 100 * 1024 * 1024,
};
const DEFAULT_LOCAL_API: LocalApiConfig = {
  enabled: false,
  hostOnNetwork: false,
  port: 4892,
};

function createSettingsState() {
  let loaded = $state(false);
  let loading: Promise<void> | null = null;

  let peerRotation = $state<PeerRotationInterval>('manual');
  let typingIndicators = $state(true);
  let readReceipts = $state(false);
  let relay = $state<RelayConfig>({ ...DEFAULT_RELAY });
  let listeningPort = $state<number | null>(null);
  let nearbyShare = $state(true);
  let autoAccept = $state<AutoAcceptConfig>({ ...DEFAULT_AUTO_ACCEPT });
  let autoDownload = $state<AutoDownloadConfig>({ ...DEFAULT_AUTO_DOWNLOAD });
  let localApi = $state<LocalApiConfig>({ ...DEFAULT_LOCAL_API });
  let nodes = $state<NodesResponse>({ defaults: [], custom: [] });

  function load(): Promise<void> {
    if (loaded) return Promise.resolve();
    if (loading) return loading;
    loading = (async () => {
      const results = await Promise.allSettled([
        getPeerRotationInterval(),
        getTypingIndicatorsEnabled(),
        getReadReceiptsEnabled(),
        getRelayConfig(),
        getListeningPort(),
        getNearbyShareEnabled(),
        getAutoAcceptConfig(),
        getAutoDownloadConfig(),
        getLocalApiConfig(),
      ]);
      const [pr, ti, rr, rc, lp, ns, aa, ad, la] = results;
      if (pr.status === 'fulfilled') peerRotation = pr.value;
      if (ti.status === 'fulfilled') typingIndicators = ti.value;
      if (rr.status === 'fulfilled') readReceipts = rr.value;
      if (rc.status === 'fulfilled') relay = rc.value;
      if (lp.status === 'fulfilled') listeningPort = lp.value;
      if (ns.status === 'fulfilled') nearbyShare = ns.value;
      if (aa.status === 'fulfilled') autoAccept = aa.value;
      if (ad.status === 'fulfilled') autoDownload = ad.value;
      if (la.status === 'fulfilled') localApi = la.value;
      for (const r of results) {
        if (r.status === 'rejected') log.error('settings preload', r.reason);
      }
      loaded = true;
    })();
    return loading;
  }

  const setPeerRotation = (v: PeerRotationInterval) =>
    optimistic(
      () => peerRotation,
      (x) => (peerRotation = x),
      setPeerRotationInterval,
      v
    );
  const setTyping = (v: boolean) =>
    optimistic(
      () => typingIndicators,
      (x) => (typingIndicators = x),
      setTypingIndicatorsEnabled,
      v
    );
  const setReadReceipts = (v: boolean) =>
    optimistic(
      () => readReceipts,
      (x) => (readReceipts = x),
      setReadReceiptsEnabled,
      v
    );
  const setRelay = (v: RelayConfig) =>
    optimistic(
      () => relay,
      (x) => (relay = x),
      setRelayConfig,
      v
    );
  // Stored as 0 for "any port", but the command takes null.
  const setPort = (v: number | null) =>
    optimistic(
      () => listeningPort,
      (x) => (listeningPort = x),
      () => setListeningPort(v),
      v ?? 0
    );
  const setNearby = (v: boolean) =>
    optimistic(
      () => nearbyShare,
      (x) => (nearbyShare = x),
      setNearbyShareEnabled,
      v
    );
  const setAutoAccept = (v: AutoAcceptConfig) =>
    optimistic(
      () => autoAccept,
      (x) => (autoAccept = x),
      setAutoAcceptConfig,
      v
    );
  const setAutoDownload = (v: AutoDownloadConfig) =>
    optimistic(
      () => autoDownload,
      (x) => (autoDownload = x),
      setAutoDownloadConfig,
      v
    );
  const setLocalApi = (v: LocalApiConfig) =>
    optimistic(
      () => localApi,
      (x) => (localApi = x),
      setLocalApiConfig,
      v
    );

  async function loadNodes() {
    nodes = await getNodes();
  }
  async function addNode(addr: string) {
    await addCustomNode(addr);
    await loadNodes();
  }
  const removeNode = (addr: string) =>
    optimistic(
      () => nodes,
      (x) => (nodes = x),
      () => removeCustomNode(addr),
      { ...nodes, custom: nodes.custom.filter((n) => n !== addr) }
    );

  return {
    get loaded() {
      return loaded;
    },
    get peerRotation() {
      return peerRotation;
    },
    get typingIndicators() {
      return typingIndicators;
    },
    get readReceipts() {
      return readReceipts;
    },
    get relay() {
      return relay;
    },
    get listeningPort() {
      return listeningPort;
    },
    get nearbyShare() {
      return nearbyShare;
    },
    get autoAccept() {
      return autoAccept;
    },
    get autoDownload() {
      return autoDownload;
    },
    get localApi() {
      return localApi;
    },
    get nodes() {
      return nodes;
    },
    load,
    loadNodes,
    addNode,
    removeNode,
    setPeerRotation,
    setTyping,
    setReadReceipts,
    setRelay,
    setPort,
    setNearby,
    setAutoAccept,
    setAutoDownload,
    setLocalApi,
  };
}

export const settingsState = createSettingsState();
