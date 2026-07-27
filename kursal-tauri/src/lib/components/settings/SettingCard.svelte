<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    title,
    description,
    tone = 'default',
    children,
    footer,
  }: {
    title?: string;
    description?: string;
    tone?: 'default' | 'danger';
    children: Snippet;
    footer?: Snippet;
  } = $props();
</script>

<section class="card-wrap">
  {#if title || description}
    <header class="card-head">
      {#if title}<h3 class="card-title">{title}</h3>{/if}
      {#if description}<p class="card-desc">{description}</p>{/if}
    </header>
  {/if}

  <div class="card" data-tone={tone}>
    {@render children()}
  </div>

  {#if footer}
    <footer class="card-foot">
      {@render footer()}
    </footer>
  {/if}
</section>

<style>
  .card-wrap {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .card-head {
    margin-bottom: 10px;
    padding: 0 4px;
  }
  .card-title {
    margin: 0 0 3px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-primary);
  }
  .card-desc {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.5;
  }
  .card {
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    overflow: hidden;
  }
  .card[data-tone='danger'] {
    border-color: rgba(239, 68, 68, 0.3);
    background: var(--danger-dim);
  }
  .card-foot {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 10px;
    padding: 0 4px;
  }
</style>
