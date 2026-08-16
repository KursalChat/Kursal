<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { fade, slide } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
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
  import { ensurePermission } from '$lib/api/permissions';
  import { nearbyState } from '$lib/state/nearby.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { settingsState } from '$lib/state/settings.svelte';
  import { t } from '$lib/i18n';
  import type { NearbyOrigin } from '$lib/types';
  import { Bluetooth, Radar, Wifi } from 'lucide-svelte';

  let connecting = $state<Set<string>>(new Set());
  let declining = $state<Set<string>>(new Set());
  let bluetoothDenied = $state(false);
  let pollInterval: ReturnType<typeof setInterval> | null = null;

  const mdnsDisabled = $derived(settingsState.loaded && !settingsState.nearbyShare);
  const requests = $derived(Object.entries(nearbyState.pendingRequests));
  const total = $derived(nearbyState.peers.length + requests.length);

  function setBusy(set: Set<string>, peerId: string, isBusy: boolean) {
    const next = new Set(set);
    if (isBusy) next.add(peerId);
    else next.delete(peerId);
    return next;
  }

  onMount(async () => {
    try {
      await settingsState.load();
    } catch (e) {
      log.error('Settings load failed:', e);
    }
    bluetoothDenied = !(await ensurePermission('bluetooth'));
    try {
      const sessionName = await startNearby();
      nearbyState.active = true;
      nearbyState.mySessionName = sessionName;
      nearbyState.setPeers(await getNearbyPeers());

      pollInterval = setInterval(async () => {
        try {
          nearbyState.setPeers(await getNearbyPeers());
        } catch (e) {
          log.error('Failed to get nearby peers:', e);
        }
      }, 3000);
    } catch (e) {
      notifications.push(t('addContact.nearby.discoveryFailed'), 'error');
      log.error('Start nearby failed:', e);
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
    connecting = setBusy(connecting, peerId, true);
    try {
      await connectNearby(peerId, origin);
      notifications.push(t('addContact.nearby.connectionSent'), 'info');
    } catch (e) {
      notifications.push(t('addContact.nearby.connectionFailed'), 'error');
      log.error('Connect failed:', e);
    } finally {
      connecting = setBusy(connecting, peerId, false);
    }
  }

  async function handleAccept(peerId: string) {
    connecting = setBusy(connecting, peerId, true);
    try {
      await acceptNearby(peerId);
      nearbyState.removePendingRequest(peerId);
      notifications.push(t('addContact.nearby.connectionAccepted'), 'success');
    } catch (e) {
      notifications.push(t('addContact.nearby.acceptFailed'), 'error');
      log.error('Accept failed:', e);
    } finally {
      connecting = setBusy(connecting, peerId, false);
    }
  }

  async function handleDecline(peerId: string) {
    declining = setBusy(declining, peerId, true);
    try {
      await declineNearby(peerId);
      nearbyState.removePendingRequest(peerId);
      notifications.push(t('addContact.nearby.connectionDeclined'), 'info');
    } catch (e) {
      log.error('Decline failed:', e);
      notifications.push(t('addContact.nearby.declineFailed'), 'error');
    } finally {
      declining = setBusy(declining, peerId, false);
    }
  }
</script>

<section class="strip" class:populated={total > 0}>
  <div class="head">
    <Radar size={15} />
    <span class="label">{t('addContact.connect.rowNearby')}</span>
    <span class="who">
      {nearbyState.mySessionName
        ? t('addContact.nearby.yourName', { name: nearbyState.mySessionName })
        : t('addContact.nearby.startingDiscovery')}
    </span>
    <span class="status" class:live={nearbyState.active}>
      {#if nearbyState.active && total === 0}<span class="dot"></span>{/if}
      {total > 0 ? t('addContact.nearby.found', { count: total }) : t('addContact.nearby.scanning')}
    </span>
  </div>

  {#if mdnsDisabled}
    <p class="warn">{t('addContact.nearby.wifiDisabledDescription')}</p>
  {/if}

  {#if bluetoothDenied}
    <p class="warn">{t('addContact.nearby.bluetoothDeniedDescription')}</p>
  {/if}

  {#if total > 0}
    <div class="peers" transition:slide={{ duration: 200, easing: cubicOut }}>
      {#each requests as [peerId, sessionName] (peerId)}
        <div class="chip incoming" transition:fade={{ duration: 120 }}>
          <span class="chip-name">{sessionName}</span>
          <span class="chip-sub">{t('addContact.nearby.wantsToConnectShort')}</span>
          <button
            class="chip-action decline"
            type="button"
            disabled={connecting.has(peerId) || declining.has(peerId)}
            onclick={() => handleDecline(peerId)}
          >
            {t('addContact.nearby.declineButton')}
          </button>
          <button
            class="chip-action accept"
            type="button"
            disabled={connecting.has(peerId) || declining.has(peerId)}
            onclick={() => handleAccept(peerId)}
          >
            {t('addContact.nearby.acceptButton')}
          </button>
        </div>
      {/each}

      {#each nearbyState.peers as peer (`${peer.peerId}:${peer.origin}`)}
        <button
          class="chip peer"
          type="button"
          disabled={connecting.has(peer.peerId)}
          onclick={() => handleConnect(peer.peerId, peer.origin)}
          transition:fade={{ duration: 120 }}
          animate:flip={{ duration: 200 }}
        >
          {#if peer.origin === 'Bluetooth'}
            <Bluetooth size={12} strokeWidth={2.5} />
          {:else}
            <Wifi size={12} strokeWidth={2.5} />
          {/if}
          <span class="chip-name">{peer.sessionName}</span>
          <span class="chip-action accept">{t('addContact.nearby.connectButton')}</span>
        </button>
      {/each}
    </div>
  {/if}
</section>

<style>
  .strip {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    padding: 10px 12px;
  }

  .strip.populated {
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .head > :global(svg) {
    flex-shrink: 0;
    color: var(--accent);
  }

  .label {
    font-size: 13px;
    font-weight: 700;
    color: var(--text-primary);
    flex-shrink: 0;
  }

  .who {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
  }

  .status.live {
    color: var(--text-secondary);
  }

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--accent);
    animation: live-pulse 1.8s ease-out infinite;
  }

  @keyframes live-pulse {
    0% {
      box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 55%, transparent);
    }
    70% {
      box-shadow: 0 0 0 5px color-mix(in srgb, var(--accent) 0%, transparent);
    }
    100% {
      box-shadow: 0 0 0 0 color-mix(in srgb, var(--accent) 0%, transparent);
    }
  }

  .warn {
    margin: 8px 0 0;
    font-size: 11px;
    line-height: 1.45;
    color: var(--warning);
  }

  .peers {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 10px;
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    padding: 6px 7px 6px 10px;
    font-size: 12px;
    color: var(--text-secondary);
    transition:
      background var(--transition),
      border-color var(--transition);
  }

  .chip :global(svg) {
    flex-shrink: 0;
    color: var(--text-muted);
  }

  .chip.peer:hover:not(:disabled) {
    background: var(--bg-hover);
    border-color: var(--accent);
  }

  .chip:disabled {
    opacity: 0.55;
  }

  .chip.incoming {
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    background: var(--accent-dim);
  }

  .chip-name {
    font-weight: 600;
    color: var(--text-primary);
  }

  .chip-sub {
    color: var(--text-muted);
  }

  .chip-action {
    display: inline-flex;
    align-items: center;
    border-radius: var(--radius-sm);
    padding: 3px 8px;
    font-size: 11px;
    font-weight: 700;
    transition: opacity var(--transition);
  }

  .chip-action.accept {
    background: var(--accent-solid);
    color: #fff;
  }

  .chip-action.decline {
    color: var(--text-secondary);
  }

  button.chip-action:hover:not(:disabled) {
    opacity: 0.85;
  }

  button.chip-action.decline:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  @media (max-width: 640px) {
    .head {
      flex-wrap: wrap;
    }

    .who {
      order: 3;
      flex: 1 0 100%;
      margin-top: 4px;
    }

    .status {
      margin-left: auto;
    }

    .chip {
      width: 100%;
    }

    .chip.peer .chip-action {
      margin-left: auto;
    }

    .chip.incoming .chip-action.decline {
      margin-left: auto;
    }
  }
</style>
