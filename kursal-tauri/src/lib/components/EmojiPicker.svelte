<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { X, Search } from 'lucide-svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { trapFocus } from '$lib/utils/focusTrap';
  import { readInsets } from '$lib/utils/android-insets';
  import { t } from '$lib/i18n';
  import {
    loadEmojiIndex,
    searchEmojis,
    getTone,
    setTone,
    applyTone,
    bumpRecent,
    topRecents,
    type EmojiIndex,
    type Emoji,
    type ToneId,
  } from '$lib/emoji';

  let {
    onSelect,
    onClose,
    compact = false,
    autoFocus = true,
  }: {
    onSelect: (emoji: string, keepOpen: boolean) => void;
    onClose: () => void;
    compact?: boolean;
    autoFocus?: boolean;
  } = $props();

  let index = $state.raw<EmojiIndex | null>(null);
  let tone = $state<ToneId>(getTone());
  let searchQuery = $state('');
  let activeCategory = $state('recent');
  let toneOpen = $state(false);
  let pickerEl = $state<HTMLDivElement | null>(null);
  let gridEl = $state<HTMLDivElement | null>(null);
  let searchInput = $state<HTMLInputElement | null>(null);
  let toneSelectorEl = $state<HTMLDivElement | null>(null);

  const TONE_REF: Emoji = {
    unicode: '✋',
    label: 'raised hand',
    group: 'people-body',
    tags: [],
    shortcodes: [],
    skins: [
      { tone: 1, unicode: '✋🏻' },
      { tone: 2, unicode: '✋🏼' },
      { tone: 3, unicode: '✋🏽' },
      { tone: 4, unicode: '✋🏾' },
      { tone: 5, unicode: '✋🏿' },
    ],
  };

  const TONE_SWATCHES: { tone: ToneId; unicode: string }[] = [
    { tone: 0, unicode: '✋' },
    { tone: 1, unicode: '✋🏻' },
    { tone: 2, unicode: '✋🏼' },
    { tone: 3, unicode: '✋🏽' },
    { tone: 4, unicode: '✋🏾' },
    { tone: 5, unicode: '✋🏿' },
  ];

  function onSearchKeydown(e: KeyboardEvent) {
    if (e.key !== 'Tab' || e.shiftKey) return;
    const first = gridEl?.querySelector<HTMLElement>('.emoji-btn');
    if (!first) return;
    e.preventDefault();
    first.focus();
  }

  function adjustPosition() {
    if (!pickerEl) return;
    pickerEl.style.setProperty('--x-shift', '0px');
    const rect = pickerEl.getBoundingClientRect();
    const safe = readInsets();
    const margin = 8;
    const right = window.innerWidth - safe.right - margin;
    const left = safe.left + margin;
    let shift = 0;
    if (rect.right > right) shift = right - rect.right;
    if (rect.left + shift < left) shift = left - rect.left;
    pickerEl.style.setProperty('--x-shift', `${shift}px`);
  }

  onMount(() => {
    loadEmojiIndex().then((idx) => {
      index = idx;
    });

    tick().then(() => {
      // Skip on touch: focusing the search box re-summons the on-screen
      // keyboard, which would cover the picker we just opened.
      if (autoFocus) searchInput?.focus();
      adjustPosition();
    });

    function handleClickOutside(e: MouseEvent) {
      if (pickerEl && !pickerEl.contains(e.target as Node)) {
        onClose();
      }
    }
    function handleEscape(e: KeyboardEvent) {
      if (e.key === 'Escape') onClose();
    }
    function handleResize() {
      adjustPosition();
    }

    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleEscape);
    window.addEventListener('resize', handleResize);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
      window.removeEventListener('resize', handleResize);
    };
  });

  const recents = $derived(topRecents(compact ? 7 : 8));
  const results = $derived(
    index && searchQuery.trim() ? searchEmojis(index.flat, searchQuery) : null
  );

  function pick(unicode: string, keepOpen: boolean) {
    bumpRecent(unicode);
    onSelect(unicode, keepOpen);
  }

  function chooseTone(tn: ToneId) {
    tone = tn;
    setTone(tn);
    toneOpen = false;
  }

  // Sections use content-visibility:auto, so off-screen heights are estimates
  // and a single scroll lands short; re-measure per frame until the target
  // sits flush (or we give up).
  function scrollToCategory(id: string) {
    activeCategory = id;
    const container = gridEl;
    if (!container) return;
    let tries = 0;
    function step() {
      const section = container!.querySelector<HTMLElement>(`[data-category="${id}"]`);
      if (!section) return;
      const delta = section.getBoundingClientRect().top - container!.getBoundingClientRect().top;
      container!.scrollTop += delta;
      if (Math.abs(delta) > 1 && tries++ < 40) requestAnimationFrame(step);
    }
    step();
  }

  // All tabs: recent synthetic group (only when recents exist) + loaded groups
  const allTabs = $derived.by(() => {
    const groups = index?.groups ?? [];
    const recentTab =
      recents.length > 0 ? [{ id: 'recent', icon: '🕐', label: t('emojiPicker.recent') }] : [];
    return [
      ...recentTab,
      ...groups.map((g) => ({ id: g.id, icon: g.icon, label: t('emojiPicker.groups.' + g.id) })),
    ];
  });

  $effect(() => {
    if (!toneOpen) return;
    function handleToneOutside(e: MouseEvent) {
      if (toneSelectorEl && !toneSelectorEl.contains(e.target as Node)) {
        toneOpen = false;
      }
    }
    document.addEventListener('mousedown', handleToneOutside);
    return () => document.removeEventListener('mousedown', handleToneOutside);
  });
</script>

<div class="emoji-picker" class:compact bind:this={pickerEl} use:trapFocus={{ returnFocus: false }}>
  <div class="picker-header">
    <div class="search-box">
      <Search size={14} />
      <input
        type="text"
        placeholder={t('emojiPicker.searchPlaceholder')}
        bind:value={searchQuery}
        bind:this={searchInput}
        onkeydown={onSearchKeydown}
      />
      {#if searchQuery}
        <button
          class="clear-search"
          onclick={() => {
            searchQuery = '';
            searchInput?.focus();
          }}
        >
          <X size={12} />
        </button>
      {/if}
    </div>

    {#if index}
      <div class="tone-selector" bind:this={toneSelectorEl}>
        <button
          class="tone-trigger"
          aria-label={t('emojiPicker.skinTone')}
          onclick={() => (toneOpen = !toneOpen)}>{applyTone(TONE_REF, tone)}</button
        >
        {#if toneOpen}
          <div class="tone-swatches">
            {#each TONE_SWATCHES as s (s.tone)}
              <button
                class="tone-swatch"
                class:active={tone === s.tone}
                onclick={() => chooseTone(s.tone)}>{s.unicode}</button
              >
            {/each}
          </div>
        {/if}
      </div>
    {/if}
  </div>

  {#if !index}
    <div class="loading-state">
      <Spinner size={16} />
      <span>{t('emojiPicker.loading')}</span>
    </div>
  {:else}
    {#if !results}
      <div class="category-tabs">
        {#each allTabs as tab (tab.id)}
          <button
            class="cat-tab"
            class:active={activeCategory === tab.id}
            onclick={() => scrollToCategory(tab.id)}
            title={tab.label}>{tab.icon}</button
          >
        {/each}
      </div>
    {/if}

    <div class="emoji-grid-container" bind:this={gridEl}>
      {#if results}
        {#if results.length === 0}
          <div class="no-results">{t('emojiPicker.noResults')}</div>
        {:else}
          <div class="emoji-grid">
            {#each results as emoji (emoji.unicode)}
              <button
                class="emoji-btn"
                title={emoji.label}
                onclick={(e) => pick(applyTone(emoji, tone), e.shiftKey)}
                >{applyTone(emoji, tone)}</button
              >
            {/each}
          </div>
        {/if}
      {:else}
        {#if recents.length > 0}
          <div class="category-section" data-category="recent">
            <div class="category-label">{t('emojiPicker.recent')}</div>
            <div class="emoji-grid">
              {#each recents as u (u)}
                <button class="emoji-btn" onclick={(e) => pick(u, e.shiftKey)}>{u}</button>
              {/each}
            </div>
          </div>
        {/if}

        {#each index.groups as group (group.id)}
          <div class="category-section" data-category={group.id}>
            <div class="category-label">{t('emojiPicker.groups.' + group.id)}</div>
            <div class="emoji-grid">
              {#each group.emojis as emoji (emoji.unicode)}
                <button
                  class="emoji-btn"
                  title={emoji.label}
                  onclick={(e) => pick(applyTone(emoji, tone), e.shiftKey)}
                  >{applyTone(emoji, tone)}</button
                >
              {/each}
            </div>
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .emoji-picker {
    width: min(352px, calc(100vw - var(--safe-left) - var(--safe-right) - 16px));
    max-height: min(420px, calc(100vh - var(--safe-top) - var(--safe-bottom) - 80px));
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--glow);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    z-index: 200;
    transform: translateX(var(--x-shift, 0));
    animation: picker-pop 0.18s cubic-bezier(0.34, 1.56, 0.64, 1);
    transform-origin: bottom left;
  }
  @keyframes picker-pop {
    from {
      opacity: 0;
      transform: translateX(var(--x-shift, 0)) scale(0.88);
    }
    to {
      opacity: 1;
      transform: translateX(var(--x-shift, 0)) scale(1);
    }
  }
  .emoji-picker.compact {
    width: min(288px, calc(100vw - var(--safe-left) - var(--safe-right) - 16px));
    max-height: min(248px, calc(100vh - var(--safe-top) - var(--safe-bottom) - 40px));
    border-radius: var(--radius-md);
  }
  .emoji-picker.compact .picker-header {
    padding: 8px 8px 0;
  }
  .emoji-picker.compact .search-box {
    padding: 6px 8px;
    border-radius: 8px;
  }
  .emoji-picker.compact .search-box input {
    font-size: var(--text-xs);
  }
  .emoji-picker.compact .category-tabs {
    padding: 6px 8px 4px;
  }
  .emoji-picker.compact .cat-tab {
    height: 28px;
    font-size: var(--text-md);
  }
  .emoji-picker.compact .emoji-grid-container {
    padding: 0 8px 8px;
  }
  .emoji-picker.compact .emoji-grid {
    grid-template-columns: repeat(7, 1fr);
  }
  .emoji-picker.compact .emoji-btn {
    font-size: 18px;
    border-radius: 6px;
  }
  .emoji-picker.compact .category-label {
    font-size: 10px;
    padding: 3px 2px 2px;
  }

  .picker-header {
    padding: 10px 10px 0;
    display: flex;
    align-items: center;
  }

  .search-box {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--surface-soft);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 8px 10px;
    color: var(--text-muted);
  }

  .search-box input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: var(--text-sm);
  }

  .search-box input::placeholder {
    color: var(--text-muted);
  }

  .clear-search {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: var(--bg-hover);
    color: var(--text-muted);
    cursor: pointer;
    transition: background 0.15s;
  }

  @media (hover: hover) {
    .clear-search:hover {
      background: color-mix(in srgb, var(--bg-hover) 70%, var(--text-muted));
    }
  }

  .category-tabs {
    display: flex;
    gap: 1px;
    padding: 8px 6px 6px;
    border-bottom: 1px solid var(--border);
  }

  .cat-tab {
    flex: 1 1 0;
    min-width: 0;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: var(--text-lg);
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.15s;
    line-height: 1;
  }

  @media (hover: hover) {
    .cat-tab:hover {
      background: var(--bg-hover);
    }
  }

  .cat-tab.active {
    background: var(--accent-dim);
  }

  .emoji-grid-container {
    flex: 1;
    overflow-y: auto;
    padding: 0 10px 10px;
  }

  .category-section {
    margin-bottom: 8px;
    content-visibility: auto;
    contain-intrinsic-size: 0 240px;
  }

  .category-label {
    font-size: var(--text-2xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    padding: 4px 4px 3px;
    position: sticky;
    top: 0;
    background: var(--bg-secondary);
    z-index: 1;
  }

  .emoji-grid {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 2px;
  }

  .emoji-btn {
    width: 100%;
    aspect-ratio: 1;
    font-size: 22px;
    border-radius: 8px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition:
      background 0.12s,
      transform 0.12s;
    line-height: 1;
  }

  @media (hover: hover) {
    .emoji-btn:hover {
      background: var(--bg-hover);
      transform: scale(1.15);
    }
  }

  .no-results {
    text-align: center;
    color: var(--text-muted);
    font-size: var(--text-sm);
    padding: 32px 0;
  }

  .loading-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    color: var(--text-muted);
    font-size: var(--text-sm);
    padding: 32px 0;
  }

  .tone-selector {
    position: relative;
    margin-left: 6px;
    flex-shrink: 0;
  }

  .tone-trigger {
    width: 32px;
    height: 32px;
    font-size: 18px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: background 0.12s;
  }

  @media (hover: hover) {
    .tone-trigger:hover {
      background: var(--bg-hover);
    }
  }

  .tone-swatches {
    position: absolute;
    right: 0;
    top: calc(100% + 6px);
    display: flex;
    gap: 2px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 4px;
    box-shadow: var(--glow);
    z-index: 210;
  }

  .tone-swatch {
    width: 26px;
    height: 26px;
    border-radius: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 16px;
    cursor: pointer;
    transition: background 0.12s;
  }

  @media (hover: hover) {
    .tone-swatch:hover {
      background: var(--bg-hover);
    }
  }

  .tone-swatch.active {
    background: var(--accent-dim);
  }

  @media (max-width: 500px) {
    .emoji-picker {
      max-height: min(360px, calc(100vh - var(--safe-top) - var(--safe-bottom) - 60px));
    }
  }
</style>
