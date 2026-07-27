<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    description,
    disabled = false,
    children,
  }: {
    title: string;
    description?: string;
    disabled?: boolean;
    children: Snippet;
  } = $props();
</script>

<div class="row" data-disabled={disabled}>
  <div class="label-col">
    <span class="title">{title}</span>
    {#if description}
      <span class="desc">{description}</span>
    {/if}
  </div>
  <div class="control-col">
    {@render children()}
  </div>
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border-light);
  }
  .row:last-child {
    border-bottom: none;
  }
  .row[data-disabled='true'] {
    opacity: 0.55;
  }
  .label-col {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .desc {
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.45;
  }
  .control-col {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 8px;
  }
</style>
