<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { fade } from 'svelte/transition';
  import { flip } from 'svelte/animate';
  import { log } from '$lib/utils/log';
  import {
    startNearby,
    stopNearby,
    getNearbyPeers,
    connectNearby,
    acceptNearby,
    declineNearby,
  } from '$lib/api/nearby';
  import { nearbyState } from '$lib/state/nearby.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { settingsState } from '$lib/state/settings.svelte';
  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n';
  import type { NearbyOrigin } from '$lib/types';
  import { Bluetooth, Wifi, WifiOff } from 'lucide-svelte';

  const mdnsDisabled = $derived(settingsState.loaded && !settingsState.nearbyShare);

  let connecting = $state<Set<string>>(new Set());
  let declining = $state<Set<string>>(new Set());
  let pollInterval: ReturnType<typeof setInterval> | null = null;
  let isInitialLoad = $state(true);

  function setConnecting(peerId: string, isBusy: boolean) {
    const next = new Set(connecting);
    if (isBusy) next.add(peerId);
    else next.delete(peerId);
    connecting = next;
  }

  function setDeclining(peerId: string, isBusy: boolean) {
    const next = new Set(declining);
    if (isBusy) next.add(peerId);
    else next.delete(peerId);
    declining = next;
  }

  onMount(async () => {
    try {
      await settingsState.load();
    } catch (e) {
      log.error('Settings load failed:', e);
    }
    try {
      const sessionName = await startNearby();
      nearbyState.active = true;
      nearbyState.mySessionName = sessionName;

      const peers = await getNearbyPeers();
      nearbyState.setPeers(peers);
      isInitialLoad = false;

      pollInterval = setInterval(async () => {
        try {
          const nextPeers = await getNearbyPeers();
          nearbyState.setPeers(nextPeers);
        } catch (e) {
          log.error('Failed to get nearby peers:', e);
        }
      }, 3000);
    } catch (e) {
      notifications.push(t('addContact.nearby.discoveryFailed'), 'error');
      log.error('Start nearby failed:', e);
      isInitialLoad = false;
    }
  });

  onDestroy(async () => {
    if (pollInterval) clearInterval(pollInterval);
    try {
      await stopNearby();
    } catch (e) {
      log.error('Stop nearby failed:', e);
    }
    nearbyState.reset();
  });

  async function handleConnect(peerId: string, origin: NearbyOrigin) {
    setConnecting(peerId, true);
    try {
      await connectNearby(peerId, origin);
      notifications.push(t('addContact.nearby.connectionSent'), 'info');
    } catch (e) {
      notifications.push(t('addContact.nearby.connectionFailed'), 'error');
      log.error('Connect failed:', e);
    } finally {
      setConnecting(peerId, false);
    }
  }

  async function handleAccept(peerId: string) {
    setConnecting(peerId, true);
    try {
      await acceptNearby(peerId);
      nearbyState.removePendingRequest(peerId);
      notifications.push(t('addContact.nearby.connectionAccepted'), 'success');
    } catch (e) {
      notifications.push(t('addContact.nearby.acceptFailed'), 'error');
      log.error('Accept failed:', e);
    } finally {
      setConnecting(peerId, false);
    }
  }

  async function handleDecline(peerId: string) {
    setDeclining(peerId, true);
    try {
      await declineNearby(peerId);
      nearbyState.removePendingRequest(peerId);
      notifications.push(t('addContact.nearby.connectionDeclined'), 'info');
    } catch (e) {
      log.error('Decline failed:', e);
      notifications.push(t('addContact.nearby.declineFailed'), 'error');
    } finally {
      setDeclining(peerId, false);
    }
  }
</script>

<div class="nearby-page">
  {#if mdnsDisabled}
    <section class="banner">
      <WifiOff size={16} strokeWidth={2.25} />
      <div class="banner-text">
        <p class="banner-title">{t('addContact.nearby.wifiDisabled')}</p>
        <p class="banner-body">
          {t('addContact.nearby.wifiDisabledDescription')}
        </p>
      </div>
    </section>
  {/if}

  <section class="identity-card">
    <div>
      <p class="kicker">{t('addContact.nearby.yourName')}</p>
      <h3>
        {nearbyState.mySessionName || t('addContact.nearby.startingDiscovery')}
      </h3>
    </div>
    <span class="status" class:active={nearbyState.active}>
      {#if nearbyState.active}<span class="live-dot"></span>{/if}
      {nearbyState.active ? t('addContact.nearby.scanning') : t('addContact.nearby.stopped')}
    </span>
  </section>

  {#if Object.keys(nearbyState.pendingRequests).length > 0}
    <section class="panel warning">
      <div class="panel-header">
        <h4>{t('addContact.nearby.incomingRequests')}</h4>
      </div>
      <div class="row-list">
        {#each Object.entries(nearbyState.pendingRequests) as [peerId, sessionName]}
          <div class="row-item">
            <div class="row-info">
              <p class="row-title">{sessionName}</p>
              <p class="row-subtitle">
                {t('addContact.nearby.wantsToConnect', {
                  peerId: `${peerId.slice(0, 14)}...`,
                })}
              </p>
            </div>
            <div class="row-actions">
              <Button
                variant="secondary"
                loading={declining.has(peerId)}
                disabled={connecting.has(peerId)}
                onclick={() => handleDecline(peerId)}
              >
                {t('addContact.nearby.declineButton')}
              </Button>
              <Button
                variant="primary"
                loading={connecting.has(peerId)}
                disabled={declining.has(peerId)}
                onclick={() => handleAccept(peerId)}
              >
                {t('addContact.nearby.acceptButton')}
              </Button>
            </div>
          </div>
        {/each}
      </div>
    </section>
  {/if}

  <section class="panel">
    <div class="panel-header">
      <h4>{t('addContact.nearby.devicesNearby')}</h4>
      <p>
        {t('addContact.nearby.found', { count: nearbyState.peers.length })}
      </p>
    </div>

    {#if isInitialLoad}
      <div class="empty scanning">
        <span class="radar"></span>
        <span>{t('addContact.nearby.scanningNetwork')}</span>
      </div>
    {:else if nearbyState.peers.length === 0}
      <div class="empty">{t('addContact.nearby.noDevices')}</div>
    {:else}
      <div class="row-list">
        {#each nearbyState.peers as peer (`${peer.peerId}:${peer.origin}`)}
          <div
            class="row-item"
            transition:fade={{ duration: 150 }}
            animate:flip={{ duration: 200 }}
          >
            <div class="row-info">
              <p class="row-title">
                {peer.sessionName}
                <span class="origin-badge" class:bluetooth={peer.origin === 'Bluetooth'}>
                  {#if peer.origin === 'Bluetooth'}
                    <Bluetooth size={11} strokeWidth={2.5} />
                    {t('addContact.nearby.bluetooth')}
                  {:else}
                    <Wifi size={11} strokeWidth={2.5} />
                    {t('addContact.nearby.wifi')}
                  {/if}
                </span>
              </p>
              <p class="row-subtitle">{peer.peerId.slice(0, 14)}...</p>
            </div>
            <Button
              variant="primary"
              loading={connecting.has(peer.peerId)}
              onclick={() => handleConnect(peer.peerId, peer.origin)}
            >
              {t('addContact.nearby.connectButton')}
            </Button>
          </div>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .nearby-page {
    max-width: 760px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .identity-card {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    padding: 14px 16px;
  }

  .banner {
    display: flex;
    gap: 12px;
    align-items: flex-start;
    border: 1px solid color-mix(in srgb, var(--warning) 40%, transparent);
    background: color-mix(in srgb, var(--warning) 12%, transparent);
    color: var(--text-primary);
    border-radius: var(--radius-md);
    padding: 12px 14px;
  }

  .banner :global(svg) {
    color: var(--warning);
    flex-shrink: 0;
    margin-top: 2px;
  }

  .banner-text {
    min-width: 0;
  }

  .banner-title {
    margin: 0;
    font-size: 13px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .banner-body {
    margin: 3px 0 0;
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.45;
  }

  .kicker {
    margin: 0;
    font-size: 11px;
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.1em;
  }

  .identity-card h3 {
    margin: 4px 0 0;
    font-size: 16px;
    font-weight: 700;
    color: var(--text-primary);
    letter-spacing: -0.01em;
  }

  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    border: 1px solid var(--border);
    background: var(--bg-input);
    border-radius: var(--radius-md);
    padding: 5px 10px;
  }

  .live-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
    animation: live-pulse 1.6s ease-out infinite;
  }
  @keyframes live-pulse {
    0% {
      box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 55%, transparent);
    }
    70% {
      box-shadow: 0 0 0 6px color-mix(in srgb, var(--accent) 0%, transparent);
    }
    100% {
      box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 0%, transparent);
    }
  }

  .status.active {
    color: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    background: var(--accent-dim);
  }

  .panel {
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    overflow: hidden;
  }

  .panel.warning {
    border-color: color-mix(in srgb, var(--warning) 40%, transparent);
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 1px solid var(--border-light);
    padding: 10px 14px;
    background: var(--bg-secondary);
  }

  .panel-header h4,
  .panel-header p {
    margin: 0;
  }

  .panel-header h4 {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-primary);
  }

  .panel-header p {
    font-size: 12px;
    color: var(--text-muted);
  }

  .row-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
  }

  .row-item {
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    padding: 12px;
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: center;
  }

  .row-info {
    min-width: 0;
  }

  .row-title {
    margin: 0;
    font-weight: 600;
    font-size: 13px;
    color: var(--text-primary);
    display: inline-flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .origin-badge {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--accent);
    background: var(--accent-dim);
    border: 1px solid color-mix(in srgb, var(--accent) 35%, transparent);
    padding: 2px 8px;
    border-radius: var(--radius-md);
  }

  .origin-badge.bluetooth {
    color: var(--info);
    background: rgba(56, 189, 248, 0.12);
    border-color: rgba(56, 189, 248, 0.35);
  }

  .row-subtitle {
    margin: 3px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .row-actions {
    display: flex;
    gap: 8px;
  }

  .empty {
    padding: 18px 14px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .empty.scanning {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .radar {
    position: relative;
    width: 12px;
    height: 12px;
    flex-shrink: 0;
  }
  .radar::before,
  .radar::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: var(--accent);
  }
  .radar::before {
    transform: scale(0.4);
  }
  .radar::after {
    opacity: 0.6;
    animation: radar-ping 1.8s ease-out infinite;
  }
  @keyframes radar-ping {
    0% {
      transform: scale(0.4);
      opacity: 0.6;
    }
    100% {
      transform: scale(1.7);
      opacity: 0;
    }
  }

  @media (max-width: 640px) {
    .row-item {
      flex-direction: column;
      align-items: stretch;
    }

    .row-actions {
      width: 100%;
    }

    :global(.row-actions .button),
    :global(.row-item > .button) {
      width: 100%;
    }
  }
</style>
