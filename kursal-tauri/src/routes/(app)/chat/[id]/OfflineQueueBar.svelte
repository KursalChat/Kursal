<script lang="ts">
  import { CloudUpload, CloudOff } from 'lucide-svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { t } from '$lib/i18n';

  interface Props {
    pendingUpload: number;
    inMailbox: number;
    flushing: boolean;
    contactName: string;
  }

  let { pendingUpload, inMailbox, flushing, contactName }: Props = $props();

  const total = $derived(pendingUpload + inMailbox);

  const mixed = $derived(pendingUpload > 0 && inMailbox > 0);
</script>

{#if total > 0}
  {#if flushing}
    <div class="queue-bar busy">
      <span class="icon"><Spinner size={14} color="currentColor" /></span>
      <span class="count">{total}</span>
      <span class="label">{t('chat.offlineQueue.uploading')}</span>
    </div>
  {:else if pendingUpload > 0}
    <div class="queue-bar pending" title={t('chat.offlineQueue.pendingUploadTitle')}>
      <span class="icon"><CloudUpload size={16} /></span>
      {#if mixed}
        <span class="label"
          >{t('chat.offlineQueue.mixed', { uploading: pendingUpload, stored: inMailbox })}</span
        >
      {:else}
        <span class="count">{pendingUpload}</span>
        <span class="label">{t('chat.offlineQueue.pendingUpload')}</span>
      {/if}
    </div>
  {:else}
    <div class="queue-bar stored" title={t('chat.offlineQueue.storedTitle', { name: contactName })}>
      <span class="icon"><CloudOff size={15} /></span>
      <span class="count">{inMailbox}</span>
      <span class="label">{t('chat.offlineQueue.stored', { name: contactName })}</span>
    </div>
  {/if}
{/if}

<style>
  .queue-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    align-self: center;
    margin: 0 auto;
    padding: 6px 14px;
    background: var(--surface);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid var(--border-light);
    border-radius: var(--radius-md);
    font-size: 12.5px;
    font-weight: 600;
    transition:
      transform var(--transition),
      background var(--transition);
    animation: slideUp 0.22s cubic-bezier(0.34, 1.56, 0.64, 1);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  }
  .queue-bar.pending {
    color: var(--accent-hover);
    cursor: default;
  }
  .queue-bar.stored {
    color: var(--text-secondary);
    cursor: default;
  }
  .queue-bar.busy {
    color: var(--accent-hover);
    cursor: progress;
  }
  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .queue-bar.stored .icon {
    opacity: 0.8;
  }
  .count {
    background: var(--accent);
    color: #fff;
    border-radius: var(--radius-md);
    min-width: 20px;
    padding: 0 6px;
    height: 18px;
    font-size: var(--text-2xs);
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .queue-bar.stored .count {
    background: var(--text-muted);
  }
  .label {
    letter-spacing: 0.01em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 220px;
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
