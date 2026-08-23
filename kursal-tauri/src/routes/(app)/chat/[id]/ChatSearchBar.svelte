<script lang="ts">
  import { ChevronUp, ChevronDown, X } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import { tick } from 'svelte';

  interface Props {
    query: string;
    count: number;
    current: number;
    onInput: (q: string) => void;
    onNext: () => void;
    onPrev: () => void;
    onClose: () => void;
  }

  let { query, count, current, onInput, onNext, onPrev, onClose }: Props = $props();

  let inputEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    tick().then(() => inputEl?.focus());
  });

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      onClose();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      e.shiftKey ? onPrev() : onNext();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      onNext();
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      onPrev();
    }
  }
</script>

<div class="search-bar" role="search">
  <input
    bind:this={inputEl}
    class="search-input"
    type="search"
    value={query}
    placeholder={t('chat.search.placeholder')}
    oninput={(e) => onInput((e.currentTarget as HTMLInputElement).value)}
    onkeydown={handleKeydown}
    autocomplete="off"
    spellcheck={false}
  />
  {#if query}
    <span class="search-count" aria-live="polite">
      {count === 0
        ? t('chat.search.noResults')
        : t('chat.search.count', { current: count === 0 ? 0 : current + 1, total: count })}
    </span>
  {/if}
  <div class="search-nav">
    <button
      class="nav-btn"
      onclick={onPrev}
      disabled={count === 0}
      aria-label={t('chat.search.prevAriaLabel')}><ChevronUp size={15} /></button
    >
    <button
      class="nav-btn"
      onclick={onNext}
      disabled={count === 0}
      aria-label={t('chat.search.nextAriaLabel')}><ChevronDown size={15} /></button
    >
  </div>
  <button class="nav-btn close-btn" onclick={onClose} aria-label={t('chat.search.closeAriaLabel')}
    ><X size={15} /></button
  >
</div>

<style>
  .search-bar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px max(10px, var(--safe-right)) 6px max(10px, var(--safe-left));
    background: var(--surface);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
    animation: search-in 200ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  @keyframes search-in {
    from {
      opacity: 0;
      transform: translateY(-6px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .search-input {
    flex: 1;
    min-width: 0;
    height: 30px;
    padding: 0 10px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--border);
    background: var(--bg-secondary);
    color: var(--text-primary);
    font-size: var(--text-sm);
    outline: none;
    transition: border-color var(--transition);
  }
  .search-input:focus {
    border-color: var(--accent);
  }
  .search-input::-webkit-search-cancel-button {
    display: none;
  }
  .search-count {
    font-size: var(--text-xs);
    color: var(--text-muted);
    white-space: nowrap;
    padding: 0 4px;
    font-variant-numeric: tabular-nums;
  }
  .search-nav {
    display: flex;
    gap: 2px;
  }
  .nav-btn {
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    display: flex;
    align-items: center;
    justify-content: center;
    transition:
      background var(--transition),
      color var(--transition);
  }
  @media (hover: hover) {
    .nav-btn:hover:not(:disabled) {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
  }
  .nav-btn:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .close-btn {
    margin-left: 2px;
  }
</style>
