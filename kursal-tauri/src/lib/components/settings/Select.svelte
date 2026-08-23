<script lang="ts" generics="T extends string">
  import { ChevronDown, Check } from 'lucide-svelte';
  import { readInsets } from '$lib/utils/android-insets';

  let {
    value,
    options,
    onchange,
    placeholder = 'Choose…',
    disabled = false,
    minWidth = '160px',
  }: {
    value: T | '';
    options: { value: T; label: string }[];
    onchange?: (value: T) => void;
    placeholder?: string;
    disabled?: boolean;
    minWidth?: string;
  } = $props();

  let open = $state(false);
  let highlighted = $state(-1);
  let triggerEl = $state<HTMLButtonElement | null>(null);
  let menuEl = $state<HTMLDivElement | null>(null);
  let menuStyle = $state('');

  const uid = Math.random().toString(36).slice(2);
  const menuId = `select-menu-${uid}`;
  const optionId = (i: number) => `select-opt-${uid}-${i}`;

  const currentLabel = $derived.by(() => {
    const found = options.find((o) => o.value === value);
    return found?.label ?? '';
  });

  function position() {
    if (!triggerEl) return;
    const rect = triggerEl.getBoundingClientRect();
    const safe = readInsets();
    const gap = 8;
    const wanted = Math.min(280, options.length * 34 + 8);
    const spaceBelow = window.innerHeight - safe.bottom - rect.bottom - gap;
    const spaceAbove = rect.top - safe.top - gap;
    const placeAbove = spaceBelow < wanted && spaceAbove > spaceBelow;

    const height = Math.min(wanted, Math.max(placeAbove ? spaceAbove : spaceBelow, 80));
    const top = placeAbove ? Math.max(safe.top + gap, rect.top - height - 4) : rect.bottom + 4;
    const rightEdge = window.innerWidth - safe.right - gap - rect.width;
    const left = Math.max(safe.left + gap, Math.min(rect.left, rightEdge));

    menuStyle =
      `top: ${top}px; left: ${left}px; ` + `min-width: ${rect.width}px; max-height: ${height}px;`;
  }

  function scrollHighlightedIntoView() {
    if (!menuEl || highlighted < 0) return;
    menuEl
      .querySelector<HTMLElement>(`[data-index="${highlighted}"]`)
      ?.scrollIntoView({ block: 'nearest' });
  }

  function openMenu() {
    if (disabled) return;
    open = true;
    highlighted = Math.max(
      0,
      options.findIndex((o) => o.value === value)
    );
    queueMicrotask(() => {
      position();
      scrollHighlightedIntoView();
    });
  }

  function pick(v: T) {
    onchange?.(v);
    open = false;
    triggerEl?.focus();
  }

  function moveHighlight(dir: number) {
    const n = options.length;
    if (n === 0) return;
    highlighted = (highlighted + dir + n) % n;
    scrollHighlightedIntoView();
  }

  function onDocClick(e: MouseEvent) {
    if (!open) return;
    const target = e.target as Node;
    if (triggerEl?.contains(target) || menuEl?.contains(target)) return;
    open = false;
  }

  function onKey(e: KeyboardEvent) {
    if (disabled) return;
    if (!open) {
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        openMenu();
      }
      return;
    }
    switch (e.key) {
      case 'Escape':
        e.preventDefault();
        open = false;
        break;
      case 'ArrowDown':
        e.preventDefault();
        moveHighlight(1);
        break;
      case 'ArrowUp':
        e.preventDefault();
        moveHighlight(-1);
        break;
      case 'Home':
        e.preventDefault();
        highlighted = 0;
        scrollHighlightedIntoView();
        break;
      case 'End':
        e.preventDefault();
        highlighted = options.length - 1;
        scrollHighlightedIntoView();
        break;
      case 'Enter':
      case ' ':
        e.preventDefault();
        if (highlighted >= 0 && highlighted < options.length) pick(options[highlighted].value);
        break;
      case 'Tab':
        open = false;
        break;
    }
  }

  $effect(() => {
    if (!open) return;
    const close = () => (open = false);
    document.addEventListener('mousedown', onDocClick, true);
    window.addEventListener('resize', close);
    window.addEventListener('scroll', close, true);
    return () => {
      document.removeEventListener('mousedown', onDocClick, true);
      window.removeEventListener('resize', close);
      window.removeEventListener('scroll', close, true);
    };
  });
</script>

<div class="select-wrap" style="--select-min-width: {minWidth}">
  <button
    bind:this={triggerEl}
    type="button"
    class="trigger"
    data-open={open}
    {disabled}
    role="combobox"
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls={open ? menuId : undefined}
    aria-activedescendant={open && highlighted >= 0 ? optionId(highlighted) : undefined}
    onclick={() => (open ? (open = false) : openMenu())}
    onkeydown={onKey}
  >
    <span class="label" data-placeholder={!currentLabel}>
      {currentLabel || placeholder}
    </span>
    <ChevronDown size={14} />
  </button>
</div>

{#if open}
  <div bind:this={menuEl} id={menuId} class="menu" role="listbox" style={menuStyle}>
    {#each options as opt, i (opt.value)}
      <button
        type="button"
        class="item"
        id={optionId(i)}
        data-index={i}
        role="option"
        aria-selected={opt.value === value}
        data-selected={opt.value === value}
        data-highlighted={i === highlighted}
        onmouseenter={() => (highlighted = i)}
        onclick={() => pick(opt.value)}
      >
        <span>{opt.label}</span>
        {#if opt.value === value}<Check size={14} />{/if}
      </button>
    {/each}
  </div>
{/if}

<style>
  .select-wrap {
    position: relative;
    display: inline-block;
    min-width: min(var(--select-min-width, 160px), 100%);
    max-width: 100%;
  }
  .trigger {
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--bg-input);
    color: var(--text-primary);
    font-size: var(--text-sm);
    font-weight: 500;
    transition:
      border-color var(--transition),
      background var(--transition);
  }
  @media (hover: hover) {
    .trigger:hover:not(:disabled) {
      background: var(--bg-hover);
    }
  }
  .trigger[data-open='true'] {
    border-color: var(--accent);
  }
  .trigger:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .label {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .trigger :global(svg) {
    flex-shrink: 0;
  }
  .label[data-placeholder='true'] {
    color: var(--text-muted);
  }

  .menu {
    position: fixed;
    overflow-y: auto;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 4px;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
    z-index: 1000;
  }
  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    border-radius: 6px;
    font-size: var(--text-sm);
    font-weight: 500;
    color: var(--text-secondary);
    text-align: left;
  }
  .item[data-highlighted='true'] {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  @media (hover: hover) {
    .item:hover {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
  }
  .item[data-selected='true'] {
    color: var(--text-primary);
    background: var(--accent-dim);
  }
</style>
