<script lang="ts">
  import { Share2, Trash2 } from 'lucide-svelte';
  import AddressChip from '$lib/components/AddressChip.svelte';
  import { t } from '$lib/i18n';

  export type NodeState = 'up' | 'down' | 'unknown';

  let {
    addr,
    state,
    onShare,
    onRemove,
  }: {
    addr: string;
    state: NodeState;
    onShare: () => void;
    onRemove?: () => void;
  } = $props();
</script>

<div class="node-row">
  <span class="node-dot {state}" title={t(`settings.network.nodeState_${state}`)}></span>
  <AddressChip {addr} />
  <button class="node-btn" aria-label={t('settings.network.shareNodeAriaLabel')} onclick={onShare}>
    <Share2 size={14} />
  </button>
  {#if onRemove}
    <button
      class="node-btn danger"
      aria-label={t('settings.network.removeNodeAriaLabel')}
      onclick={onRemove}
    >
      <Trash2 size={14} />
    </button>
  {/if}
</div>

<style>
  .node-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-input);
    transition: border-color var(--transition);
  }
  .node-btn {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    padding: 4px;
    border-radius: var(--radius-sm);
    transition: color var(--transition);
  }
  @media (hover: hover) {
    .node-btn:hover {
      color: var(--accent);
    }
    .node-btn.danger:hover {
      color: var(--danger);
    }
  }
  .node-dot {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--text-secondary);
  }
  .node-dot.up {
    background: var(--success);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--success) 22%, transparent);
  }
  .node-dot.down {
    background: var(--danger);
  }
  .node-dot.unknown {
    background: var(--text-secondary);
    opacity: 0.5;
  }
</style>
