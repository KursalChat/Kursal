<script lang="ts">
  import { t } from '$lib/i18n';
  import { X } from 'lucide-svelte';
  import Spinner from '$lib/components/Spinner.svelte';

  interface Props {
    name: string;
    sharing: boolean;
    onShare: () => void;
    onDismiss: () => void;
  }

  let { name, sharing, onShare, onDismiss }: Props = $props();
</script>

<div class="share-banner">
  <span>{t('chat.conversation.shareBanner', { name })}</span>
  <div class="banner-actions">
    <button class="banner-btn primary" onclick={onShare} disabled={sharing}>
      {#if sharing}
        <Spinner size={13} color="currentColor" />
      {/if}
      {t('chat.conversation.shareBannerButton')}
    </button>
    <button
      class="banner-btn icon"
      onclick={onDismiss}
      aria-label={t('chat.conversation.dismissBannerAriaLabel')}><X size={14} /></button
    >
  </div>
</div>

<style>
  .share-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 10px max(16px, var(--safe-right)) 10px max(16px, var(--safe-left));
    background: var(--accent-dim);
    font-size: var(--text-sm);
    color: var(--text-secondary);
    flex-shrink: 0;
    animation: share-in 280ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  @keyframes share-in {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .banner-actions {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .banner-btn {
    padding: 6px 12px;
    border-radius: var(--radius-sm);
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-secondary);
    transition: all var(--transition);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
  }
  .banner-btn.icon {
    padding: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  @media (hover: hover) {
    .banner-btn:hover:not(:disabled) {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
  }
  .banner-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .banner-btn.primary {
    background: var(--accent);
    color: #fff;
  }
  @media (hover: hover) {
    .banner-btn.primary:hover {
      background: var(--accent-hover);
    }
  }
</style>
