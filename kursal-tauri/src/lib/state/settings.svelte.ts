import { log } from '$lib/utils/log';
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
  enabled: false,
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

  async function setPeerRotation(v: PeerRotationInterval) {
    const prev = peerRotation;
    peerRotation = v;
    try {
      await setPeerRotationInterval(v);
    } catch (e) {
      peerRotation = prev;
      throw e;
    }
  }
  async function setTyping(v: boolean) {
    const prev = typingIndicators;
    typingIndicators = v;
    try {
      await setTypingIndicatorsEnabled(v);
    } catch (e) {
      typingIndicators = prev;
      throw e;
    }
  }
  async function setReadReceipts(v: boolean) {
    const prev = readReceipts;
    readReceipts = v;
    try {
      await setReadReceiptsEnabled(v);
    } catch (e) {
      readReceipts = prev;
      throw e;
    }
  }
  async function setRelay(v: RelayConfig) {
    const prev = relay;
    relay = v;
    try {
      await setRelayConfig(v);
    } catch (e) {
      relay = prev;
      throw e;
    }
  }
  async function setPort(v: number | null) {
    const prev = listeningPort;
    listeningPort = v ?? 0;
    try {
      await setListeningPort(v);
    } catch (e) {
      listeningPort = prev;
      throw e;
    }
  }
  async function setNearby(v: boolean) {
    const prev = nearbyShare;
    nearbyShare = v;
    try {
      await setNearbyShareEnabled(v);
    } catch (e) {
      nearbyShare = prev;
      throw e;
    }
  }
  async function setAutoAccept(v: AutoAcceptConfig) {
    const prev = autoAccept;
    autoAccept = v;
    try {
      await setAutoAcceptConfig(v);
    } catch (e) {
      autoAccept = prev;
      throw e;
    }
  }
  async function setAutoDownload(v: AutoDownloadConfig) {
    const prev = autoDownload;
    autoDownload = v;
    try {
      await setAutoDownloadConfig(v);
    } catch (e) {
      autoDownload = prev;
      throw e;
    }
  }
  async function setLocalApi(v: LocalApiConfig) {
    const prev = localApi;
    localApi = v;
    try {
      await setLocalApiConfig(v);
    } catch (e) {
      localApi = prev;
      throw e;
    }
  }

  async function loadNodes() {
    nodes = await getNodes();
  }
  async function addNode(addr: string) {
    await addCustomNode(addr);
    await loadNodes();
  }
  async function removeNode(addr: string) {
    const prev = nodes;
    nodes = { ...nodes, custom: nodes.custom.filter((n) => n !== addr) };
    try {
      await removeCustomNode(addr);
    } catch (e) {
      nodes = prev;
      throw e;
    }
  }

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
