<script lang="ts">
  import { fly } from 'svelte/transition';
  import { notifications } from '$lib/state/notifications.svelte';
  import { t } from '$lib/i18n';
</script>

<div class="container">
  {#each notifications.toasts as toast, index (toast.id)}
    <div
      in:fly={{ y: 10, duration: 200 }}
      out:fly={{ y: 10, duration: 150 }}
      class="toast {toast.kind}"
      role={toast.kind === 'error' ? 'alert' : 'status'}
      style={`--stack-index: ${index}; --stack-count: ${notifications.toasts.length};`}
    >
      <span>{toast.message}</span>
      {#if toast.action}
        {@const action = toast.action}
        <button
          class="toast-action"
          onclick={() => {
            action.onClick();
            notifications.dismiss(toast.id);
          }}>{action.label}</button
        >
      {/if}
      <button
        class="toast-dismiss"
        onclick={() => notifications.dismiss(toast.id)}
        aria-label={t('common.close')}>×</button
      >
    </div>
  {/each}
</div>

<style>
  .container {
    position: fixed;
    bottom: calc(24px + var(--safe-bottom));
    right: calc(24px + var(--safe-right));
    display: flex;
    flex-direction: column;
    gap: 8px;
    z-index: 9999;
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    background: var(--bg-secondary);
    border-radius: var(--radius-md);
    border-left: 3px solid;
    font-size: 14px;
    pointer-events: all;
    min-width: 240px;
    position: relative;
    transition:
      transform var(--transition),
      margin var(--transition),
      opacity var(--transition),
      box-shadow var(--transition);
    box-shadow: 0 8px 18px rgba(2, 6, 23, 0.24);
  }

  .container:not(:hover) .toast {
    --depth: calc(var(--stack-count) - var(--stack-index) - 1);
    transform: translateY(calc(var(--depth) * 6px)) scale(calc(1 - var(--depth) * 0.02));
    opacity: calc(1 - var(--depth) * 0.12);
  }

  .container:not(:hover) .toast + .toast {
    margin-top: -38px;
  }

  .container:not(:hover) .toast:not(:last-child) {
    pointer-events: none;
  }

  .container:hover .toast {
    transform: none;
    opacity: 1;
  }

  .toast.success {
    border-left-color: var(--success);
  }

  .toast.error {
    border-left-color: var(--danger);
  }

  .toast.info {
    border-left-color: var(--accent);
  }

  .toast.warning {
    border-left-color: var(--warning, #f59e0b);
  }

  .toast > span {
    flex: 1;
    min-width: 0;
  }
  .toast-action {
    flex-shrink: 0;
    font-size: 12.5px;
    font-weight: 700;
    color: var(--accent);
    padding: 2px 4px;
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }
  .toast-action:hover {
    color: var(--accent-hover);
  }
  .toast-dismiss {
    flex-shrink: 0;
    opacity: 0.5;
    font-size: 18px;
    line-height: 1;
    padding: 0;
  }
  .toast-dismiss:hover {
    opacity: 1;
  }

  @media (max-width: 640px) {
    .container {
      right: max(12px, var(--safe-right));
      left: max(12px, var(--safe-left));
      bottom: calc(12px + var(--safe-bottom));
    }

    .toast {
      min-width: 0;
      width: 100%;
    }
  }
</style>
