<script lang="ts">
  import { applyTone, getTone, type Emoji } from '$lib/emoji';
  import { t } from '$lib/i18n';

  let {
    items,
    bottom,
    left,
    selected = $bindable(0),
    onPick,
  }: {
    items: Emoji[];
    bottom: number;
    left: number;
    selected?: number;
    onPick: (u: string) => void;
  } = $props();

  const tone = getTone();
</script>

{#if items.length}
  <div
    class="sc-pop"
    style="bottom:{bottom}px; left:{left}px;"
    role="listbox"
    aria-label={t('chat.composer.shortcodeAriaLabel')}
  >
    {#each items as e, i (e.unicode)}
      <button
        class="sc-row"
        class:sel={i === selected}
        role="option"
        aria-selected={i === selected}
        onmousedown={(ev) => {
          ev.preventDefault();
          onPick(applyTone(e, tone));
        }}
      >
        <span class="sc-em">{applyTone(e, tone)}</span>
        <span class="sc-code">:{e.shortcodes[0] ?? e.label}:</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .sc-pop {
    position: fixed;
    z-index: 1000;
    background: var(--surface);
    border: 1px solid var(--border-light);
    border-radius: var(--radius-md);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.18);
    padding: 4px;
    max-height: 240px;
    overflow-y: auto;
    min-width: 200px;
    animation: sc-in 140ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  @keyframes sc-in {
    from {
      opacity: 0;
      transform: translateY(4px) scale(0.96);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }
  .sc-row {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    text-align: left;
  }
  .sc-row.sel,
  .sc-row:hover {
    background: var(--bg-hover);
  }
  .sc-em {
    font-size: 18px;
    line-height: 1;
  }
  .sc-code {
    font-size: 12.5px;
    color: var(--text-secondary);
  }
</style>
