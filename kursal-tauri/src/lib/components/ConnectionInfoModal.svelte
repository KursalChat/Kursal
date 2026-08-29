<script lang="ts">
  import { Copy, RotateCw, Radio, Check } from 'lucide-svelte';
  import { copyText } from '$lib/utils/clipboard';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { flashSet } from '$lib/utils/flash.svelte';
  import { dialAddress } from '$lib/api/settings';
  import { sortAddresses } from '$lib/utils/multiaddr';
  import Modal from './Modal.svelte';
  import Button from './Button.svelte';
  import AddressChip from './AddressChip.svelte';
  import { t } from '$lib/i18n';
  import type { ContactResponse } from '$lib/types';

  let { contact, onClose }: { contact: ContactResponse; onClose: () => void } = $props();

  let reconnecting = $state(false);
  const copied = flashSet();

  const addresses = $derived(sortAddresses(contact.knownAddresses));
  const status = $derived(contactsState.connectionStatus[contact.userId] ?? 'disconnected');
  const statusLabel = $derived.by(() => {
    if (status === 'direct') return t('connectionInfo.transportDirect');
    if (status === 'holepunch') return t('connectionInfo.transportHolepunch');
    if (status === 'relay') return t('connectionInfo.transportRelay');
    if (status === 'connecting') return t('connectionInfo.transportConnecting');
    return t('connectionInfo.transportOffline');
  });

  async function copyAddr(addr: string) {
    if (await copyText(addr)) copied.trigger(addr);
  }

  async function reconnect() {
    if (addresses.length === 0) return;
    reconnecting = true;
    try {
      const results = await Promise.allSettled(addresses.map((a) => dialAddress(a)));
      const ok = results.some((r) => r.status === 'fulfilled');
      notifications.push(
        ok ? t('connectionInfo.reconnectSuccess') : t('connectionInfo.reconnectFailed'),
        ok ? 'success' : 'error'
      );
    } finally {
      reconnecting = false;
    }
  }
</script>

<Modal title={t('connectionInfo.heading')} {onClose} width={440} scroll>
  <h2><Radio size={18} /> {t('connectionInfo.heading')}</h2>

  <div class="status-line" data-status={status}>
    <span class="dot"></span>
    <span class="status-text">{statusLabel}</span>
  </div>

  <div class="addr-section">
    <span class="addr-label">{t('connectionInfo.knownAddresses')}</span>
    {#if addresses.length === 0}
      <p class="addr-empty">{t('connectionInfo.noAddresses')}</p>
    {:else}
      <ul class="addr-list">
        {#each addresses as addr (addr)}
          <li class="addr-row">
            <AddressChip {addr} />
            <button
              class="addr-copy"
              class:confirmed={copied.has(addr)}
              aria-label={copied.has(addr) ? t('common.copied') : t('connectionInfo.copyAriaLabel')}
              onclick={() => copyAddr(addr)}
            >
              {#if copied.has(addr)}
                <Check size={13} />
              {:else}
                <Copy size={13} />
              {/if}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <Button
    variant="secondary"
    loading={reconnecting}
    disabled={addresses.length === 0}
    onclick={reconnect}
  >
    <RotateCw size={14} />
    {t('connectionInfo.reconnectButton')}
  </Button>
  <button class="link" onclick={onClose}>{t('connectionInfo.closeButton')}</button>
</Modal>

<style>
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: var(--text-md);
    color: var(--text-primary);
  }
  .status-line {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-secondary);
  }
  .status-line .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--text-muted);
  }
  .status-line[data-status='direct'] .dot,
  .status-line[data-status='holepunch'] .dot {
    background: var(--success);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--success) 22%, transparent);
  }
  .status-line[data-status='relay'] .dot {
    background: var(--info, #4fc3f7);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--info, #4fc3f7) 22%, transparent);
  }
  .status-line[data-status='connecting'] .dot {
    background: var(--warning, #fbbf24);
  }
  .addr-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .addr-label {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .addr-empty {
    margin: 0;
    font-size: var(--text-sm);
    color: var(--text-secondary);
  }
  .addr-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .addr-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-input);
  }
  .addr-copy {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    padding: 4px;
    border-radius: var(--radius-sm, 6px);
    transition: color var(--transition);
  }
  .addr-copy.confirmed {
    color: var(--success);
  }
  @media (hover: hover) {
    .addr-copy:hover {
      color: var(--accent);
    }
  }
  .link {
    align-self: center;
    color: var(--text-secondary);
    font-size: var(--text-sm);
    padding: 4px 8px;
  }
  @media (hover: hover) {
    .link:hover {
      color: var(--text-primary);
    }
  }
</style>
