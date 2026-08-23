<script lang="ts">
  import type { Snippet } from 'svelte';
  import { X, Search, Icon } from 'lucide-svelte';
  import { trapFocus } from '$lib/utils/focusTrap';
  import { t } from '$lib/i18n';

  let {
    title,
    icon: TitleIcon,
    preview,
    query = $bindable(''),
    searchPlaceholder,
    maxHeight = 'min(80vh, calc(var(--safe-h) - 32px))',
    onClose,
    children,
  }: {
    title: string;
    icon: typeof Icon;
    preview?: string;
    query?: string;
    searchPlaceholder: string;
    maxHeight?: string;
    onClose: () => void;
    children: Snippet;
  } = $props();

  // Stops the Escape from also reaching whatever opened this dialog.
  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  }
</script>

<div class="backdrop" onclick={onClose} role="presentation"></div>
<div
  class="panel"
  style="--panel-max-height: {maxHeight}"
  role="dialog"
  aria-modal="true"
  aria-label={title}
  tabindex="-1"
  onkeydown={onKeydown}
  use:trapFocus
>
  <div class="head">
    <h3><TitleIcon size={16} /> {title}</h3>
    <button class="close" onclick={onClose} aria-label={t('common.close')}>
      <X size={16} />
    </button>
  </div>

  {#if preview}
    <p class="preview">{preview}</p>
  {/if}

  <div class="search">
    <Search size={14} />
    <input
      type="text"
      placeholder={searchPlaceholder}
      bind:value={query}
      spellcheck="false"
      autocorrect="off"
      autocapitalize="off"
    />
  </div>

  <div class="list">
    {@render children()}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: var(--scrim);
    z-index: 1000;
    animation: picker-fade 120ms ease;
  }
  .panel {
    position: fixed;
    top: var(--safe-center-y);
    left: var(--safe-center-x);
    transform: translate(-50%, -50%);
    width: min(420px, calc(var(--safe-w) - 32px));
    max-height: var(--panel-max-height);
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-3);
    z-index: 1001;
    overflow: hidden;
    animation: picker-pop 160ms cubic-bezier(0.2, 0.9, 0.3, 1.1);
  }
  @keyframes picker-fade {
    from {
      opacity: 0;
    }
  }
  @keyframes picker-pop {
    from {
      opacity: 0;
      transform: translate(-50%, -48%) scale(0.97);
    }
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px 10px;
    flex-shrink: 0;
  }
  .head h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: var(--text-sm);
    font-weight: 700;
    color: var(--text-primary);
  }
  .head :global(svg) {
    color: var(--accent);
  }
  .close {
    width: 28px;
    height: 28px;
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    transition: all var(--transition);
  }
  @media (hover: hover) {
    .close:hover {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
  }
  .preview {
    margin: 0 16px 10px;
    padding: 8px 10px;
    background: var(--bg-tertiary);
    border-radius: var(--radius-md);
    font-size: var(--text-xs);
    color: var(--text-secondary);
    max-height: 60px;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    word-break: break-word;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 16px 8px;
    padding: 7px 10px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text-muted);
  }
  .search:focus-within {
    border-color: var(--accent-selected);
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: var(--text-sm);
  }
  .list {
    overflow-y: auto;
    padding: 4px 8px 10px;
  }
</style>
