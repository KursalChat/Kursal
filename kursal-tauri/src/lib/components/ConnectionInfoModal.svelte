<script lang="ts">
  import { scale } from 'svelte/transition';
  import { Copy, RotateCw, Radio } from 'lucide-svelte';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { trapFocus } from '$lib/utils/focusTrap';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { notifyError } from '$lib/utils/errors';
  import { dialAddress } from '$lib/api/settings';
  import Button from './Button.svelte';
  import AddressChip from './AddressChip.svelte';
  import { t } from '$lib/i18n';
  import type { ContactResponse } from '$lib/types';

  let { contact, onClose }: { contact: ContactResponse; onClose: () => void } = $props();

  let reconnecting = $state(false);

  const status = $derived(contactsState.connectionStatus[contact.userId] ?? 'disconnected');
  const statusLabel = $derived.by(() => {
    if (status === 'direct') return t('connectionInfo.transportDirect');
    if (status === 'holepunch') return t('connectionInfo.transportHolepunch');
    if (status === 'relay') return t('connectionInfo.transportRelay');
    if (status === 'connecting') return t('connectionInfo.transportConnecting');
    return t('connectionInfo.transportOffline');
  });

  async function copyAddr(addr: string) {
    try {
      await writeText(addr);
      notifications.push(t('connectionInfo.copied'), 'success');
    } catch (e) {
      notifyError(e);
    }
  }

  async function reconnect() {
    if (contact.knownAddresses.length === 0) return;
    reconnecting = true;
    try {
      const results = await Promise.allSettled(contact.knownAddresses.map((a) => dialAddress(a)));
      const ok = results.some((r) => r.status === 'fulfilled');
      notifications.push(
        ok ? t('connectionInfo.reconnectSuccess') : t('connectionInfo.reconnectFailed'),
        ok ? 'success' : 'error'
      );
    } finally {
      reconnecting = false;
    }
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) onClose();
  }
</script>

<div
  class="backdrop"
  role="presentation"
  onclick={handleBackdropClick}
  onkeydown={(e) => {
    if (e.key === 'Escape') onClose();
  }}
>
  <div
    class="modal"
    in:scale={{ duration: 220, start: 0.94, opacity: 0 }}
    out:scale={{ duration: 160, start: 0.94, opacity: 0 }}
    role="dialog"
    aria-modal="true"
    aria-label={t('connectionInfo.heading')}
    tabindex="-1"
    use:trapFocus
  >
    <h2><Radio size={18} /> {t('connectionInfo.heading')}</h2>

    <div class="status-line" data-status={status}>
      <span class="dot"></span>
      <span class="status-text">{statusLabel}</span>
    </div>

    <div class="addr-section">
      <span class="addr-label">{t('connectionInfo.knownAddresses')}</span>
      {#if contact.knownAddresses.length === 0}
        <p class="addr-empty">{t('connectionInfo.noAddresses')}</p>
      {:else}
        <ul class="addr-list">
          {#each contact.knownAddresses as addr (addr)}
            <li class="addr-row">
              <AddressChip {addr} />
              <button
                class="addr-copy"
                aria-label={t('connectionInfo.copyAriaLabel')}
                onclick={() => copyAddr(addr)}
              >
                <Copy size={13} />
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>

    <Button
      variant="secondary"
      loading={reconnecting}
      disabled={contact.knownAddresses.length === 0}
      onclick={reconnect}
    >
      <RotateCw size={14} />
      {t('connectionInfo.reconnectButton')}
    </Button>
    <button class="link" onclick={onClose}>{t('connectionInfo.closeButton')}</button>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: 16px;
  }
  .modal {
    background: var(--bg-secondary, var(--surface));
    border: 1px solid var(--border);
    border-radius: var(--radius-lg, 16px);
    padding: 22px;
    width: 100%;
    max-width: 440px;
    max-height: 80vh;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.4);
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: 16px;
    color: var(--text-primary);
  }
  .status-line {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-secondary);
  }
  .status-line .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--text-muted);
  }
  .status-line[data-status='direct'] .dot {
    background: var(--success);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--success) 22%, transparent);
  }
  .status-line[data-status='relay'] .dot,
  .status-line[data-status='holepunch'] .dot {
    background: var(--info, #4fc3f7);
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
    font-size: 12px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .addr-empty {
    margin: 0;
    font-size: 13px;
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
  .addr-copy:hover {
    color: var(--accent);
  }
  .link {
    align-self: center;
    color: var(--text-secondary);
    font-size: 13px;
    padding: 4px 8px;
  }
  .link:hover {
    color: var(--text-primary);
  }
</style>
