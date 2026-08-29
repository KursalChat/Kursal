<script lang="ts">
  import type { Snippet } from 'svelte';
  import { scale } from 'svelte/transition';
  import { trapFocus } from '$lib/utils/focusTrap';

  let {
    title,
    onClose,
    width = 380,
    height,
    padding = '22px',
    scroll = false,
    dismissible = true,
    labelledBy,
    children,
  }: {
    title?: string;
    onClose: () => void;
    width?: number;
    height?: string;
    padding?: string;
    scroll?: boolean;
    dismissible?: boolean;
    labelledBy?: string;
    children: Snippet;
  } = $props();

  function onBackdropClick(e: MouseEvent) {
    if (dismissible && e.target === e.currentTarget) onClose();
  }

  function onKeydown(e: KeyboardEvent) {
    if (dismissible && e.key === 'Escape') onClose();
  }
</script>

<div class="backdrop" role="presentation" onclick={onBackdropClick} onkeydown={onKeydown}>
  <div
    class="modal"
    class:scroll
    style="--modal-width: {width}px; --modal-padding: {padding}; {height
      ? `--modal-height: ${height};`
      : ''}"
    in:scale={{ duration: 220, start: 0.94, opacity: 0 }}
    out:scale={{ duration: 160, start: 0.94, opacity: 0 }}
    role="dialog"
    aria-modal="true"
    aria-label={labelledBy ? undefined : title}
    aria-labelledby={labelledBy}
    tabindex="-1"
    use:trapFocus
  >
    {@render children()}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: max(16px, var(--safe-top)) max(16px, var(--safe-right)) max(16px, var(--safe-bottom))
      max(16px, var(--safe-left));
    animation: modal-backdrop-in 0.2s ease;
  }

  .modal {
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: var(--modal-padding);
    width: 100%;
    max-width: var(--modal-width);
    height: var(--modal-height, auto);
    display: flex;
    flex-direction: column;
    gap: 14px;
    box-shadow: var(--shadow-3);
  }

  .modal.scroll {
    max-height: min(80vh, 100%);
    overflow-y: auto;
  }

  @keyframes modal-backdrop-in {
    from {
      opacity: 0;
    }
  }
</style>
