<script lang="ts" generics="T extends string">
  import type { Icon } from 'lucide-svelte';

  let {
    value,
    options,
    onchange,
    disabled = false,
    size = 'md',
    fullWidth = false,
  }: {
    value: T;
    options: {
      value: T;
      label: string;
      icon?: typeof Icon;
      disabled?: boolean;
    }[];
    onchange?: (value: T) => void;
    disabled?: boolean;
    size?: 'sm' | 'md';
    fullWidth?: boolean;
  } = $props();
</script>

<div class="segmented" data-size={size} data-full={fullWidth}>
  {#each options as opt}
    <button
      type="button"
      class="seg"
      data-active={value === opt.value}
      disabled={disabled || opt.disabled}
      onclick={() => onchange?.(opt.value)}
    >
      {#if opt.icon}
        {@const Icon = opt.icon}
        <Icon size={size === 'sm' ? 12 : 14} />
      {/if}
      <span>{opt.label}</span>
    </button>
  {/each}
</div>

<style>
  .segmented {
    display: inline-flex;
    flex-wrap: wrap;
    padding: 3px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    gap: 2px;
  }
  .segmented[data-full='true'] {
    display: flex;
    flex-wrap: wrap;
    width: 100%;
  }
  .seg {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    padding: 6px 12px;
    border-radius: 7px;
    font-size: var(--text-sm);
    font-weight: 500;
    color: var(--text-secondary);
    transition:
      background var(--transition),
      color var(--transition);
    flex: 1 1 auto;
    min-width: 0;
  }
  .seg :global(svg) {
    flex-shrink: 0;
  }
  .seg span {
    min-width: 0;
    white-space: normal;
    overflow-wrap: anywhere;
    text-align: center;
    line-height: 1.2;
  }
  .segmented[data-size='sm'] .seg {
    padding: 4px 10px;
    font-size: var(--text-xs);
  }
  @media (hover: hover) {
    .seg:hover:not(:disabled):not([data-active='true']) {
      color: var(--text-primary);
    }
  }
  .seg[data-active='true'] {
    background: var(--bg-tertiary);
    color: var(--text-primary);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
  }
  .seg:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
