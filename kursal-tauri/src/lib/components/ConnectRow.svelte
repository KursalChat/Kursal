<script lang="ts">
  import type { Snippet } from 'svelte';
  import { slide } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';
  import { ChevronDown, Icon } from 'lucide-svelte';

  let {
    icon,
    label,
    hint,
    status,
    action,
    children,
    collapsible = true,
    showBody = true,
    open = $bindable(false),
  }: {
    icon: typeof Icon;
    label: string;
    hint?: string;
    status?: string;
    action?: Snippet;
    children?: Snippet;
    collapsible?: boolean;
    showBody?: boolean;
    open?: boolean;
  } = $props();

  const IconTag = $derived(icon);
  const bodyVisible = $derived(children != null && (collapsible ? open : showBody));
</script>

<section class="row" class:open={bodyVisible}>
  {#snippet rowHead()}
    <IconTag size={16} />
    <span class="row-text">
      <strong>{label}</strong>
      {#if hint}<span>{hint}</span>{/if}
    </span>
    {#if status}<span class="row-status">{status}</span>{/if}
    {#if action}
      <span class="row-action">{@render action()}</span>
    {/if}
    {#if collapsible}
      <ChevronDown class="chevron" size={15} />
    {/if}
  {/snippet}

  {#if collapsible}
    <button class="row-head" type="button" aria-expanded={open} onclick={() => (open = !open)}>
      {@render rowHead()}
    </button>
  {:else}
    <div class="row-head">{@render rowHead()}</div>
  {/if}

  {#if bodyVisible}
    <div class="row-body" transition:slide={{ duration: 200, easing: cubicOut }}>
      <div class="row-body-inner">
        {@render children?.()}
      </div>
    </div>
  {/if}
</section>

<style>
  .row {
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    overflow: hidden;
    transition: border-color var(--transition);
  }

  .row.open {
    border-color: var(--accent-selected);
  }

  .row-head {
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    text-align: left;
    padding: 12px 14px;
    color: var(--text-secondary);
  }

  button.row-head {
    transition: background var(--transition);
  }

  button.row-head:hover {
    background: var(--bg-hover);
  }

  .row-head > :global(svg:first-child) {
    flex-shrink: 0;
    color: var(--accent);
  }

  .row-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .row-text strong {
    font-size: 13px;
    font-weight: 700;
    color: var(--text-primary);
  }

  .row-text span {
    font-size: 12px;
    line-height: 1.4;
    color: var(--text-muted);
  }

  .row-status {
    flex-shrink: 0;
    font-size: 11px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    color: var(--text-secondary);
    border: 1px solid var(--border);
    background: var(--bg-input);
    border-radius: var(--radius-md);
    padding: 4px 8px;
    white-space: nowrap;
  }

  .row-action {
    flex-shrink: 0;
  }

  .row-action :global(.button) {
    width: auto;
    min-width: 120px;
  }

  .row-head :global(.chevron) {
    flex-shrink: 0;
    color: var(--text-muted);
    transition: transform var(--transition);
  }

  .row.open .row-head :global(.chevron) {
    transform: rotate(180deg);
  }

  .row-body-inner {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 2px 14px 14px;
  }

  .row-body-inner :global(> .button) {
    width: 100%;
  }

  @media (max-width: 560px) {
    .row-head {
      flex-wrap: wrap;
    }

    .row-action {
      width: 100%;
      order: 3;
    }

    .row-action :global(.button) {
      width: 100%;
    }
  }
</style>
