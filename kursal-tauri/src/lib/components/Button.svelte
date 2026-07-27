<script lang="ts">
  import type { Snippet } from 'svelte';
  import Spinner from './Spinner.svelte';

  let {
    variant = 'primary',
    loading = false,
    disabled = false,
    onclick,
    children,
  }: {
    variant?: 'primary' | 'secondary' | 'danger';
    loading?: boolean;
    disabled?: boolean;
    onclick?: () => void;
    children: Snippet;
  } = $props();
</script>

<button
  class="button {variant}"
  class:loading
  disabled={disabled || loading}
  {onclick}
  aria-busy={loading}
>
  {#if loading}
    <span class="spinner-overlay"><Spinner size={14} color="currentColor" /></span>
  {/if}
  <span class="label">{@render children()}</span>
</button>

<style>
  .button {
    position: relative;
    min-height: 36px;
    padding: 8px 14px;
    border-radius: var(--radius-md);
    font-size: 13px;
    font-weight: 600;
    transition:
      background var(--transition),
      transform var(--transition),
      opacity var(--transition);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    white-space: nowrap;
    border: 1px solid transparent;
  }

  .button .label {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .button.loading .label {
    opacity: 0;
  }

  .spinner-overlay {
    position: absolute;
    inset: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .button.primary {
    background: var(--accent-solid);
    color: #fff;
  }

  .button.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent-solid), white 10%);
  }

  .button.secondary {
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border-color: var(--border);
  }

  .button.secondary:hover:not(:disabled) {
    background: var(--bg-hover);
    border-color: var(--accent-selected);
  }

  .button.danger {
    background: var(--danger);
    color: #fff;
    border-color: var(--danger);
  }

  .button.danger:hover:not(:disabled) {
    background: var(--danger-hover);
    border-color: var(--danger-hover);
  }

  .button:active:not(:disabled) {
    transform: scale(0.97);
  }

  .button.secondary:active:not(:disabled) {
    background: color-mix(in srgb, var(--bg-hover) 60%, var(--bg-tertiary));
  }

  .button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
