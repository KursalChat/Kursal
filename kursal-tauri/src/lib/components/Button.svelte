<script lang="ts">
  import type { Snippet } from 'svelte';
  import { Check } from 'lucide-svelte';
  import Spinner from './Spinner.svelte';
  import { t } from '$lib/i18n';

  let {
    variant = 'primary',
    loading = false,
    success = false,
    successLabel,
    disabled = false,
    onclick,
    children,
  }: {
    variant?: 'primary' | 'secondary' | 'danger';
    loading?: boolean;
    success?: boolean;
    successLabel?: string;
    disabled?: boolean;
    onclick?: () => void;
    children: Snippet;
  } = $props();

  // The spinner wins: a save that resolves instantly shouldn't flicker both.
  const showSuccess = $derived(success && !loading);
</script>

<button
  class="button {variant}"
  class:loading
  class:success={showSuccess}
  disabled={disabled || loading}
  {onclick}
  aria-busy={loading}
>
  {#if loading}
    <span class="spinner-overlay"><Spinner size={14} color="currentColor" /></span>
  {/if}
  {#if showSuccess}
    <span class="success-overlay"><Check size={15} /></span>
    <span class="sr-only" aria-live="polite">{successLabel ?? t('common.saved')}</span>
  {/if}
  <span class="label">{@render children()}</span>
</button>

<style>
  .button {
    position: relative;
    min-height: 36px;
    padding: 8px 14px;
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
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

  .button.loading .label,
  .button.success .label {
    opacity: 0;
  }

  .spinner-overlay,
  .success-overlay {
    position: absolute;
    inset: 0;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .success-overlay {
    animation: success-pop var(--transition) ease-out;
  }

  @keyframes success-pop {
    from {
      opacity: 0;
      transform: scale(0.7);
    }
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
  }

  .button.primary {
    background: var(--accent-solid);
    color: #fff;
  }

  @media (hover: hover) {
    .button.primary:hover:not(:disabled) {
      background: color-mix(in srgb, var(--accent-solid), white 10%);
    }
  }

  .button.secondary {
    background: var(--bg-tertiary);
    color: var(--text-primary);
    border-color: var(--border);
  }

  @media (hover: hover) {
    .button.secondary:hover:not(:disabled) {
      background: var(--bg-hover);
      border-color: var(--accent-selected);
    }
  }

  .button.danger {
    background: var(--danger);
    color: #fff;
    border-color: var(--danger);
  }

  @media (hover: hover) {
    .button.danger:hover:not(:disabled) {
      background: var(--danger-hover);
      border-color: var(--danger-hover);
    }
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
