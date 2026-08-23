<script lang="ts">
  import { fade } from 'svelte/transition';
  import { t } from '$lib/i18n';
  import WinstonCard from './WinstonCard.svelte';

  // Shown once ever, on the first window close that would silently leave Kursal
  // running in the tray. Stays open until the user picks Keep or Quit.
  let {
    open,
    onKeep,
    onOptOut,
    onCancel,
  }: {
    open: boolean;
    onKeep: () => void;
    onOptOut: () => void;
    onCancel: () => void;
  } = $props();

  let confirmBtn = $state<HTMLButtonElement | null>(null);

  $effect(() => {
    if (open) confirmBtn?.focus();
  });

  function onKeydown(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      onCancel();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if open}
  <div class="backdrop" transition:fade|global={{ duration: 500 }}></div>
  <div class="tip" role="dialog" aria-label={t('layout.closeExplainer.title')}>
    <WinstonCard img="/winston.webp" alt="Winston" size={84}>
      <div class="title">{t('layout.closeExplainer.title')}</div>
      <div class="body">{t('layout.closeExplainer.body')}</div>

      <div class="actions">
        <button class="skip" onclick={onOptOut}>
          {t('layout.closeExplainer.optOut')}
        </button>
        <button class="primary" bind:this={confirmBtn} onclick={onKeep}>
          {t('layout.closeExplainer.confirm')}
        </button>
      </div>

      <div class="footer">{t('layout.closeExplainer.footer')}</div>
    </WinstonCard>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 8899;
    pointer-events: none;
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
    background: rgba(2, 6, 23, 0.45);
  }

  .tip {
    position: fixed;
    right: calc(22px + var(--safe-right));
    bottom: calc(22px + var(--safe-bottom));
    z-index: 8900;
    max-width: min(400px, calc(var(--safe-w) - 44px));
  }

  .title {
    font-size: 14px;
    font-weight: 700;
    color: var(--text-primary);
    margin-bottom: 6px;
    letter-spacing: -0.01em;
  }
  .body {
    font-size: var(--text-sm);
    line-height: 1.55;
    color: var(--text-secondary);
  }

  .actions {
    margin-top: 14px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .skip {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-muted);
    padding: 6px 4px;
    text-align: left;
    transition: color var(--transition);
  }
  @media (hover: hover) {
    .skip:hover {
      color: var(--text-secondary);
    }
  }
  .primary {
    padding: 8px 18px;
    border-radius: var(--radius-md);
    background: var(--accent-solid);
    color: #fff;
    font-size: var(--text-sm);
    font-weight: 700;
    flex-shrink: 0;
    transition:
      transform var(--transition),
      box-shadow var(--transition),
      background var(--transition);
    box-shadow: 0 4px 14px var(--accent-dim);
  }
  @media (hover: hover) {
    .primary:hover {
      background: var(--accent-hover);
      transform: translateY(-1px);
      box-shadow: 0 8px 22px var(--accent-dim);
    }
  }
  .primary:active {
    transform: translateY(0);
  }

  .footer {
    margin-top: 10px;
    font-size: var(--text-2xs);
    color: var(--text-muted);
  }

  @media (max-width: 768px) {
    .tip {
      right: max(12px, var(--safe-right));
      bottom: calc(12px + var(--safe-bottom));
      left: max(12px, var(--safe-left));
      max-width: none;
    }
  }
</style>
