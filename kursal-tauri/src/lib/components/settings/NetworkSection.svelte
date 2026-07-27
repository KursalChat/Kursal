<script lang="ts">
  import { onMount } from 'svelte';
  import { Save, Plus, Trash2, RefreshCw, Copy, Share2 } from 'lucide-svelte';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import ShareModal from '$lib/components/ShareModal.svelte';
  import {
    dialAddress,
    getNetworkStatus,
    type RelayConfig,
    type NetworkStatus,
  } from '$lib/api/settings';
  import { t } from '$lib/i18n';
  import { settingsState } from '$lib/state/settings.svelte';
  import { settingsDirty } from '$lib/state/settingsDirty.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { notifyError, parseError } from '$lib/utils/errors';
  import Button from '$lib/components/Button.svelte';
  import AddressChip from '$lib/components/AddressChip.svelte';
  import SettingCard from './SettingCard.svelte';
  import CollapsibleCard from './CollapsibleCard.svelte';
  import SettingRow from './SettingRow.svelte';
  import Toggle from './Toggle.svelte';
  import TextInput from './TextInput.svelte';

  const DEFAULT_PORT = '4891';
  let relay = $state<RelayConfig>({ ...settingsState.relay });
  let relaySaving = $state(false);
  let port = $state<string>(
    settingsState.listeningPort === null ? DEFAULT_PORT : String(settingsState.listeningPort)
  );
  let portSaving = $state(false);
  let initialized = $state(settingsState.loaded);

  onMount(async () => {
    await settingsState.load();
    if (!initialized) {
      relay = { ...settingsState.relay };
      port =
        settingsState.listeningPort === null ? DEFAULT_PORT : String(settingsState.listeningPort);
      initialized = true;
    }
    settingsState.loadNodes().catch((e) => notifyError(e));
    refreshStatus();
  });

  const relayDirty = $derived(
    relay.enabled !== settingsState.relay.enabled ||
      relay.maxConnections !== settingsState.relay.maxConnections ||
      relay.maxConnectionsPerIp !== settingsState.relay.maxConnectionsPerIp
  );
  const portDirty = $derived(
    port.trim() !==
      (settingsState.listeningPort === null ? DEFAULT_PORT : String(settingsState.listeningPort))
  );
  const nearby = $derived(settingsState.nearbyShare);

  $effect(() => {
    settingsDirty.value = relayDirty || portDirty;
    return () => {
      settingsDirty.value = false;
    };
  });

  async function saveRelay() {
    relaySaving = true;
    try {
      const clean: RelayConfig = {
        ...relay,
        maxConnections: Math.max(1, Math.floor(relay.maxConnections || 0)),
        maxConnectionsPerIp: Math.max(1, Math.floor(relay.maxConnectionsPerIp || 0)),
      };
      await settingsState.setRelay(clean);
      relay = { ...clean };
      notifications.push(t('settings.network.successRelaySaved'), 'success');
    } catch (e) {
      notifyError(e);
    } finally {
      relaySaving = false;
    }
  }

  async function savePort() {
    const trimmed = port.trim();
    let parsed: number | null = null;
    if (trimmed.length > 0) {
      const n = Number(trimmed);
      if (!Number.isInteger(n) || n < 1 || n > 65535) {
        notifications.push(t('settings.network.errorPortInvalid'), 'error');
        return;
      }
      parsed = n;
    }
    portSaving = true;
    try {
      await settingsState.setPort(parsed);
      notifications.push(t('settings.network.successPortSaved'), 'success');
    } catch (e) {
      notifyError(e);
    } finally {
      portSaving = false;
    }
  }

  async function toggleNearby(value: boolean) {
    try {
      await settingsState.setNearby(value);
    } catch (e) {
      notifyError(e);
    }
  }

  let newNode = $state('');
  let addingNode = $state(false);
  let dialAddr = $state('');
  let dialing = $state(false);
  const nodes = $derived(settingsState.nodes);

  function nodeErrorMessage(e: unknown, fallback: string): string {
    const msg = parseError(e).message;
    if (msg.includes('invalid_address')) return t('settings.network.errorInvalidAddress');
    if (msg.includes('unroutable_address')) return t('settings.network.errorUnroutableAddress');
    if (msg.includes('duplicate_node')) return t('settings.network.errorDuplicateNode');
    if (msg.includes('dial_timeout') || msg.includes('dial_cancelled'))
      return t('settings.network.errorDialTimeout');
    return fallback;
  }

  async function addNode() {
    const addr = newNode.trim();
    if (addr.length === 0) return;
    addingNode = true;
    try {
      await settingsState.addNode(addr);
      newNode = '';
      notifications.push(t('settings.network.successNodeAdded'), 'success');
    } catch (e) {
      notifications.push(nodeErrorMessage(e, t('settings.network.errorNodeFailed')), 'error');
    } finally {
      addingNode = false;
    }
  }

  async function removeNode(addr: string) {
    try {
      await settingsState.removeNode(addr);
      notifications.push(t('settings.network.successNodeRemoved'), 'success');
    } catch (e) {
      notifications.push(nodeErrorMessage(e, t('settings.network.errorNodeFailed')), 'error');
    }
  }

  async function connectOnce() {
    const addr = dialAddr.trim();
    if (addr.length === 0) return;
    dialing = true;
    try {
      await dialAddress(addr);
      dialAddr = '';
      notifications.push(t('settings.network.successDialed'), 'success');
    } catch (e) {
      notifications.push(nodeErrorMessage(e, t('settings.network.errorDialFailed')), 'error');
    } finally {
      dialing = false;
    }
  }

  let shareLink = $state<string | null>(null);
  function shareNode(addr: string) {
    shareLink = `kursal://node/${encodeURIComponent(addr)}`;
  }

  let status = $state<NetworkStatus | null>(null);
  let statusLoading = $state(false);
  let importText = $state('');
  let importing = $state(false);
  let showImport = $state(false);

  const connectedSet = $derived(new Set(status?.connectedPeers ?? []));

  function peerIdOf(addr: string): string | null {
    const match = addr.match(/\/p2p\/([^/]+)/);
    return match ? match[1] : null;
  }

  // including these strings here so that "unused translation" works
  // settings.network.nodeState_up settings.network.nodeState_down settings.network.nodeState_unknown
  function nodeState(addr: string): 'up' | 'down' | 'unknown' {
    const id = peerIdOf(addr);
    if (!id) return 'unknown';
    return connectedSet.has(id) ? 'up' : 'down';
  }

  async function refreshStatus() {
    statusLoading = true;
    try {
      status = await getNetworkStatus();
    } catch (e) {
      notifyError(e);
    } finally {
      statusLoading = false;
    }
  }

  async function copyAddr(addr: string) {
    try {
      await writeText(addr);
      notifications.push(t('settings.network.copied'), 'success');
    } catch (e) {
      notifyError(e);
    }
  }

  async function exportNodes() {
    const text = nodes.custom.join('\n');
    if (text.length === 0) return;
    try {
      await writeText(text);
      notifications.push(t('settings.network.exportCopied'), 'success');
    } catch (e) {
      notifyError(e);
    }
  }

  async function importNodes() {
    const lines = importText
      .split(/\r?\n/)
      .map((l) => l.trim())
      .filter((l) => l.length > 0);
    if (lines.length === 0) return;
    importing = true;
    const results = await Promise.allSettled(lines.map((l) => settingsState.addNode(l)));
    const added = results.filter((r) => r.status === 'fulfilled').length;
    importing = false;
    importText = '';
    showImport = false;
    notifications.push(t('settings.network.importDone', { count: String(added) }), 'success');
  }

  const upCount = (list: string[]) => list.filter((a) => nodeState(a) === 'up').length;
  const defaultsUp = $derived(upCount(nodes.defaults));
  const customUp = $derived(upCount(nodes.custom));
  const totalNodes = $derived(nodes.defaults.length + nodes.custom.length);
  const reachableNodes = $derived(defaultsUp + customUp);
  const overallState = $derived(
    !status ? 'connecting' : status.peerCount > 0 ? 'online' : 'offline'
  );
  const stateLabel = $derived(
    overallState === 'online'
      ? t('settings.network.stateOnline')
      : overallState === 'offline'
        ? t('settings.network.stateOffline')
        : t('settings.network.stateConnecting')
  );

  let nodesOpen = $state(true);
  let advancedOpen = $state(false);
  let relayOpen = $state(false);
</script>

<div class="sec-head">
  <h2>{t('settings.network.heading')}</h2>
  <p>{t('settings.network.description')}</p>
</div>

<div class="net-status" data-state={overallState}>
  <div class="net-status-main">
    <span class="net-dot"></span>
    <div class="net-status-text">
      <span class="net-state">{stateLabel}</span>
      <span class="net-sub">
        {t('settings.network.statusLine', {
          count: String(status?.peerCount ?? 0),
          up: String(reachableNodes),
          total: String(totalNodes),
        })}
      </span>
    </div>
  </div>
  <button class="refresh-btn" onclick={refreshStatus} disabled={statusLoading}>
    <RefreshCw size={14} class={statusLoading ? 'spin' : ''} />
    {t('settings.network.refresh')}
  </button>
</div>

<CollapsibleCard
  title={t('settings.network.nodesCard')}
  description={t('settings.network.nodesDescription')}
  bind:open={nodesOpen}
>
  {#snippet right()}
    {#if totalNodes > 0}
      <span class="summary-pill" class:ok={reachableNodes > 0}>
        {t('settings.network.groupUpSummary', {
          up: String(reachableNodes),
          total: String(totalNodes),
        })}
      </span>
    {/if}
  {/snippet}

  <div class="card-body">
    <div class="node-group">
      <div class="node-group-head">
        <span class="node-group-label">{t('settings.network.defaultNodesLabel')}</span>
        <span class="summary-muted">
          {t('settings.network.groupUpSummary', {
            up: String(defaultsUp),
            total: String(nodes.defaults.length),
          })}
        </span>
      </div>
      {#each nodes.defaults as addr (addr)}
        <div class="node-row">
          <span
            class="node-dot {nodeState(addr)}"
            title={t(`settings.network.nodeState_${nodeState(addr)}`)}
          ></span>
          <AddressChip {addr} />
          <button
            class="node-remove"
            aria-label={t('settings.network.shareNodeAriaLabel')}
            onclick={() => shareNode(addr)}
          >
            <Share2 size={14} />
          </button>
        </div>
      {/each}
    </div>

    <div class="node-group">
      <div class="node-group-head">
        <span class="node-group-label">{t('settings.network.customNodesLabel')}</span>
        <div class="node-group-actions">
          {#if nodes.custom.length > 0}
            <span class="summary-muted">
              {t('settings.network.groupUpSummary', {
                up: String(customUp),
                total: String(nodes.custom.length),
              })}
            </span>
          {/if}
          <button class="node-link" disabled={nodes.custom.length === 0} onclick={exportNodes}>
            {t('settings.network.exportButton')}
          </button>
          <button class="node-link" onclick={() => (showImport = !showImport)}>
            {t('settings.network.importButton')}
          </button>
        </div>
      </div>

      {#if showImport}
        <div class="node-import">
          <textarea
            class="node-import-area"
            placeholder={t('settings.network.importPlaceholder')}
            bind:value={importText}></textarea>
          <Button
            onclick={importNodes}
            loading={importing}
            disabled={importText.trim().length === 0}
          >
            {t('settings.network.importConfirm')}
          </Button>
        </div>
      {/if}

      {#if nodes.custom.length === 0}
        <span class="node-empty">{t('settings.network.emptyCustomNodes')}</span>
      {:else}
        {#each nodes.custom as addr (addr)}
          <div class="node-row">
            <span
              class="node-dot {nodeState(addr)}"
              title={t(`settings.network.nodeState_${nodeState(addr)}`)}
            ></span>
            <AddressChip {addr} />
            <button
              class="node-remove"
              aria-label={t('settings.network.shareNodeAriaLabel')}
              onclick={() => shareNode(addr)}
            >
              <Share2 size={14} />
            </button>
            <button
              class="node-remove danger"
              aria-label={t('settings.network.removeNodeAriaLabel')}
              onclick={() => removeNode(addr)}
            >
              <Trash2 size={14} />
            </button>
          </div>
        {/each}
      {/if}
    </div>

    <div class="node-add">
      <TextInput
        width="100%"
        placeholder={t('settings.network.addNodePlaceholder')}
        ariaLabel={t('settings.network.addNodeAriaLabel')}
        bind:value={newNode}
      />
      <Button onclick={addNode} loading={addingNode} disabled={newNode.trim().length === 0}>
        <Plus size={13} />
        {t('settings.network.addNodeButton')}
      </Button>
    </div>
  </div>
</CollapsibleCard>

<SettingCard title={t('settings.network.discoveryCard')}>
  <SettingRow
    title={t('settings.network.nearbyShareRow')}
    description={t('settings.network.nearbyShareDescription')}
  >
    <Toggle
      checked={nearby}
      onchange={toggleNearby}
      ariaLabel={t('settings.network.nearbyShareAriaLabel')}
    />
  </SettingRow>
</SettingCard>

<CollapsibleCard
  title={t('settings.network.advancedCard')}
  description={t('settings.network.advancedDescription')}
  bind:open={advancedOpen}
>
  <div class="card-body">
    <div class="field">
      <div class="field-head">
        <span class="field-title">{t('settings.network.listeningPortRow')}</span>
        <span class="field-desc">{t('settings.network.listeningPortDescription')}</span>
      </div>
      <div class="field-control">
        <TextInput
          type="text"
          placeholder={t('settings.network.portPlaceholder')}
          width="110px"
          bind:value={port}
        />
        <Button onclick={savePort} loading={portSaving} disabled={!portDirty}>
          {t('settings.network.savePortButton')}
        </Button>
      </div>
    </div>

    <div class="node-group">
      <span class="node-group-label">{t('settings.network.listenAddresses')}</span>
      {#if !status || status.listenAddresses.length === 0}
        <span class="node-empty">{t('settings.network.noListenAddresses')}</span>
      {:else}
        {#each status.listenAddresses as addr (addr)}
          <div class="node-row">
            <AddressChip {addr} />
            <button
              class="node-remove"
              aria-label={t('settings.network.copyAriaLabel')}
              onclick={() => copyAddr(addr)}
            >
              <Copy size={14} />
            </button>
          </div>
        {/each}
      {/if}
    </div>

    <div class="connect-once">
      <div class="connect-once-head">
        <span class="connect-once-title">{t('settings.network.connectOnceRow')}</span>
        <span class="connect-once-desc">{t('settings.network.connectOnceDescription')}</span>
      </div>
      <div class="node-add">
        <TextInput
          width="100%"
          placeholder={t('settings.network.connectOncePlaceholder')}
          ariaLabel={t('settings.network.connectOnceAriaLabel')}
          bind:value={dialAddr}
        />
        <Button
          variant="secondary"
          onclick={connectOnce}
          loading={dialing}
          disabled={dialAddr.trim().length === 0}
        >
          {t('settings.network.connectOnceButton')}
        </Button>
      </div>
    </div>
  </div>
</CollapsibleCard>

<CollapsibleCard
  title={t('settings.network.relayCard')}
  description={t('settings.network.relayDescription')}
  bind:open={relayOpen}
>
  <SettingRow
    title={t('settings.network.runAsRelayRow')}
    description={t('settings.network.runAsRelayDescription')}
  >
    <Toggle
      checked={relay.enabled}
      onchange={(v) => (relay = { ...relay, enabled: v })}
      ariaLabel={t('settings.network.runAsRelayAriaLabel')}
    />
  </SettingRow>
  {#if relay.enabled}
    <SettingRow
      title={t('settings.network.maxConnectionsRow')}
      description={t('settings.network.maxConnectionsDescription')}
    >
      <TextInput
        type="number"
        min={1}
        max={100000}
        width="96px"
        value={String(relay.maxConnections)}
        onchange={(v) => (relay = { ...relay, maxConnections: Number(v) || 0 })}
      />
    </SettingRow>
    <SettingRow
      title={t('settings.network.maxPerIpRow')}
      description={t('settings.network.maxPerIpDescription')}
    >
      <TextInput
        type="number"
        min={1}
        max={10000}
        width="96px"
        value={String(relay.maxConnectionsPerIp)}
        onchange={(v) => (relay = { ...relay, maxConnectionsPerIp: Number(v) || 0 })}
      />
    </SettingRow>
  {/if}
  {#snippet footer()}
    <Button onclick={saveRelay} loading={relaySaving} disabled={!relayDirty}>
      <Save size={13} />
      {t('settings.network.saveRelayButton')}
    </Button>
  {/snippet}
</CollapsibleCard>

{#if shareLink}
  <ShareModal
    link={shareLink}
    title={t('settings.network.shareNodeTitle')}
    onClose={() => (shareLink = null)}
  />
{/if}

<style>
  .card-body {
    padding: 14px;
  }
  .card-body > :first-child {
    margin-top: 0;
  }
  .node-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 12px;
  }
  .node-group-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .node-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-input);
    transition: border-color var(--transition);
  }
  .node-row:hover {
    border-color: var(--accent-selected, var(--border));
  }
  .node-remove {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    padding: 4px;
    border-radius: var(--radius-sm, 6px);
    transition: color var(--transition);
  }
  .node-remove:hover {
    color: var(--accent);
  }
  .node-remove.danger:hover {
    color: var(--danger, #e5484d);
  }
  .node-empty {
    font-size: 13px;
    color: var(--text-secondary);
  }
  .node-add {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 12px;
  }
  .node-add :global(.ks-input) {
    flex: 1;
    min-width: 0;
  }
  .connect-once {
    margin-top: 18px;
    padding-top: 16px;
    border-top: 1px solid var(--border);
  }
  .connect-once-head {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .connect-once-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .connect-once-desc {
    font-size: 12px;
    color: var(--text-secondary);
  }
  .node-dot {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-secondary);
  }
  .node-dot.up {
    background: var(--success, #30a46c);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--success, #30a46c) 22%, transparent);
  }
  .node-dot.down {
    background: var(--danger, #e5484d);
  }
  .node-dot.unknown {
    background: var(--text-secondary);
    opacity: 0.5;
  }
  .node-group-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .node-group-actions {
    display: flex;
    gap: 12px;
  }
  .node-link {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
    font-weight: 600;
    color: var(--accent);
    transition: opacity var(--transition);
  }
  .node-link:hover {
    opacity: 0.8;
  }
  .node-link:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .node-import {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .node-import-area {
    width: 100%;
    min-height: 84px;
    resize: vertical;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--bg-input);
    color: var(--text-primary);
    padding: 8px 10px;
    font-family: var(--font-mono, monospace);
    font-size: 12px;
    outline: none;
  }
  .node-import-area:focus {
    border-color: var(--accent);
  }
  .net-status {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 13px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
  }
  .net-status-main {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }
  .net-dot {
    flex-shrink: 0;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--text-muted);
  }
  .net-status[data-state='online'] .net-dot {
    background: var(--success);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--success) 20%, transparent);
  }
  .net-status[data-state='offline'] .net-dot {
    background: var(--danger);
  }
  .net-status[data-state='connecting'] .net-dot {
    background: var(--warning);
    animation: net-pulse 1.2s ease-in-out infinite;
  }
  @keyframes net-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.4;
    }
  }
  .net-status-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .net-state {
    font-size: 14px;
    font-weight: 700;
    color: var(--text-primary);
    letter-spacing: -0.01em;
  }
  .net-sub {
    font-size: 12px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .refresh-btn {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-input);
    transition:
      color var(--transition),
      border-color var(--transition);
  }
  .refresh-btn:hover:not(:disabled) {
    color: var(--text-primary);
    border-color: var(--accent-selected);
  }
  .refresh-btn:disabled {
    opacity: 0.5;
  }
  .refresh-btn :global(.spin) {
    animation: node-spin 0.9s linear infinite;
  }

  .summary-pill {
    font-size: 11px;
    font-weight: 700;
    color: var(--text-secondary);
    background: var(--bg-subtle, rgba(127, 127, 127, 0.12));
    padding: 2px 8px;
    border-radius: 999px;
    font-variant-numeric: tabular-nums;
  }
  .summary-pill.ok {
    color: var(--success);
    background: color-mix(in srgb, var(--success) 12%, transparent);
  }
  .summary-muted {
    flex-shrink: 0;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  .field-head {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .field-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .field-desc {
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.45;
  }
  .field-control {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  @keyframes node-spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
