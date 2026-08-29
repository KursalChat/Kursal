<script lang="ts">
  import { onMount } from 'svelte';
  import { Copy } from 'lucide-svelte';
  import { copyText } from '$lib/utils/clipboard';
  import { renderQrDataUrl } from '$lib/utils/qr';
  import { flash } from '$lib/utils/flash.svelte';
  import Modal from './Modal.svelte';
  import Button from './Button.svelte';
  import { t } from '$lib/i18n';

  let { link, title, onClose }: { link: string; title: string; onClose: () => void } = $props();

  let qrDataUrl = $state<string | null>(null);
  const copied = flash();

  onMount(async () => {
    qrDataUrl = await renderQrDataUrl(link);
  });

  async function copyLink() {
    await copyText(link, { flash: copied });
  }
</script>

<Modal {title} {onClose}>
  <h2>{title}</h2>

  <div class="qr-card">
    {#if qrDataUrl}
      <img class="qr-image" src={qrDataUrl} alt={t('share.qrAlt')} />
    {:else}
      <p class="qr-fallback">{t('share.qrUnavailable')}</p>
    {/if}
  </div>

  <code class="link-text">{link}</code>

  <Button
    variant="secondary"
    onclick={copyLink}
    success={copied.active}
    successLabel={t('common.copied')}
  >
    <Copy size={14} />
    {t('share.copyLink')}
  </Button>
  <button class="link" onclick={onClose}>{t('share.close')}</button>
</Modal>

<style>
  h2 {
    margin: 0;
    font-size: var(--text-md);
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
    font-size: var(--text-sm);
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
    font-size: var(--text-sm);
    padding: 4px 8px;
  }
  @media (hover: hover) {
    .link:hover {
      color: var(--text-primary);
    }
  }
</style>
