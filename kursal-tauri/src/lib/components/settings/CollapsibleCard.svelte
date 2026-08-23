<script lang="ts">
  import type { Snippet } from 'svelte';
  import { slide } from 'svelte/transition';
  import { ChevronDown } from 'lucide-svelte';

  let {
    title,
    description,
    open = $bindable(false),
    children,
    right,
    footer,
  }: {
    title: string;
    description?: string;
    open?: boolean;
    children: Snippet;
    right?: Snippet;
    footer?: Snippet;
  } = $props();
</script>

<section class="card-wrap">
  <button class="card-head" aria-expanded={open} onclick={() => (open = !open)}>
    <div class="head-text">
      <h3 class="card-title">{title}</h3>
      {#if description}<p class="card-desc">{description}</p>{/if}
    </div>
    {#if right}<div class="head-right">{@render right()}</div>{/if}
    <ChevronDown size={16} class="chev {open ? 'open' : ''}" />
  </button>

  {#if open}
    <div class="card" transition:slide={{ duration: 200 }}>
      {@render children()}
    </div>
    {#if footer}
      <footer class="card-foot" transition:slide={{ duration: 200 }}>
        {@render footer()}
      </footer>
    {/if}
  {/if}
</section>

<style>
  .card-wrap {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .card-head {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    text-align: left;
    padding: 4px 4px 10px;
    color: var(--text-secondary);
    transition: color var(--transition);
  }
  @media (hover: hover) {
    .card-head:hover {
      color: var(--text-primary);
    }
  }
  .head-text {
    flex: 1;
    min-width: 0;
  }
  .card-title {
    margin: 0 0 3px;
    font-size: var(--text-2xs);
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-primary);
  }
  .card-desc {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
    line-height: 1.5;
  }
  .head-right {
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }
  :global(.card-head .chev) {
    flex-shrink: 0;
    color: var(--text-muted);
    transition: transform var(--transition);
  }
  :global(.card-head .chev.open) {
    transform: rotate(180deg);
  }
  .card {
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    overflow: hidden;
  }
  .card-foot {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 10px;
    padding: 0 4px;
  }
</style>
