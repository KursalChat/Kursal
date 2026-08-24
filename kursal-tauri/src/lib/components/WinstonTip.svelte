<script lang="ts">
  import { fade } from 'svelte/transition';
  import DOMPurify from 'dompurify';
  import { t } from '$lib/i18n';
  import { winstonTips } from '$lib/state/winstonTips.svelte';
  import WinstonCard from './WinstonCard.svelte';

  function escapeHtml(s: string) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }
  function formatBody(s: string) {
    const marked = escapeHtml(s)
      .replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>')
      .replace(/\*(.+?)\*/g, '<em>$1</em>');
    return DOMPurify.sanitize(marked, { ALLOWED_TAGS: ['strong', 'em'], ALLOWED_ATTR: [] });
  }

  const def = $derived(winstonTips.def);
</script>

{#if def}
  <div class="backdrop" transition:fade|global={{ duration: 500 }}></div>
  {#key winstonTips.activeId}
    <div class="tip" role="dialog" aria-label={t(def.titleKey)}>
      <WinstonCard img={def.img} alt="Winston" size={84}>
        <div class="title">{t(def.titleKey)}</div>
        <div class="body">{@html formatBody(t(def.bodyKey))}</div>

        <div class="actions">
          <button class="skip" onclick={() => winstonTips.dismiss()}>
            {t(def.dismissKey)}
          </button>
          {#if def.ctaKey}
            <button class="primary" onclick={() => winstonTips.confirm()}>
              {t(def.ctaKey)}
            </button>
          {/if}
        </div>
      </WinstonCard>
    </div>
  {/key}
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
    margin-bottom: 5px;
    letter-spacing: -0.01em;
  }
  .body {
    font-size: var(--text-sm);
    line-height: 1.55;
    color: var(--text-secondary);
  }
  .body :global(strong) {
    color: var(--text-primary);
    font-weight: 700;
  }

  .actions {
    margin-top: 12px;
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
    transition: color var(--transition);
  }
  @media (hover: hover) {
    .skip:hover {
      color: var(--text-secondary);
    }
  }
  .primary {
    padding: 8px 16px;
    border-radius: var(--radius-md);
    background: var(--accent-solid);
    color: #fff;
    font-size: var(--text-sm);
    font-weight: 700;
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

  @media (max-width: 768px) {
    .tip {
      right: max(12px, var(--safe-right));
      bottom: calc(12px + var(--safe-bottom));
      left: max(12px, var(--safe-left));
      max-width: none;
    }
  }
</style>
