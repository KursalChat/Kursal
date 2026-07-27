<script lang="ts">
  import { onMount } from 'svelte';
  import { scale } from 'svelte/transition';
  import QRCode from 'qrcode';
  import { Copy } from 'lucide-svelte';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { log } from '$lib/utils/log';
  import { trapFocus } from '$lib/utils/focusTrap';
  import { notifications } from '$lib/state/notifications.svelte';
  import { notifyError } from '$lib/utils/errors';
  import Button from './Button.svelte';
  import { t } from '$lib/i18n';

  let { link, title, onClose }: { link: string; title: string; onClose: () => void } = $props();

  let qrDataUrl = $state<string | null>(null);

  onMount(async () => {
    try {
      qrDataUrl = await QRCode.toDataURL(link, {
        errorCorrectionLevel: 'M',
        margin: 1,
        width: 320,
        color: { dark: '#0f172a', light: '#f8fafc' },
      });
    } catch (e) {
      qrDataUrl = null;
      log.error('Failed to render share QR:', e);
    }
  });

  async function copyLink() {
    try {
      await writeText(link);
      notifications.push(t('share.copied'), 'success');
    } catch (e) {
      notifyError(e);
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
    aria-label={title}
    tabindex="-1"
    use:trapFocus
  >
    <h2>{title}</h2>

    <div class="qr-card">
      {#if qrDataUrl}
        <img class="qr-image" src={qrDataUrl} alt={t('share.qrAlt')} />
      {:else}
        <p class="qr-fallback">{t('share.qrUnavailable')}</p>
      {/if}
    </div>

    <code class="link-text">{link}</code>

    <Button variant="secondary" onclick={copyLink}>
      <Copy size={14} />
      {t('share.copyLink')}
    </Button>
    <button class="link" onclick={onClose}>{t('share.close')}</button>
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
    max-width: 380px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.4);
  }
  h2 {
    margin: 0;
    font-size: 16px;
    color: var(--text-primary);
  }
  .qr-card {
    display: flex;
    align-items: center;
    justify-content: center;
    background: #f8fafc;
    border-radius: var(--radius-md);
    padding: 12px;
  }
  .qr-image {
    width: 240px;
    height: 240px;
    max-width: 100%;
    image-rendering: pixelated;
  }
  .qr-fallback {
    margin: 0;
    color: #0f172a;
    font-size: 13px;
  }
  .link-text {
    overflow-wrap: anywhere;
    font-family: var(--font-mono, monospace);
    font-size: 11.5px;
    color: var(--text-secondary);
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 8px 10px;
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
