<script lang="ts">
  import { Pin, List, X, ChevronUp, ChevronDown } from 'lucide-svelte';
  import type { MessageResponse } from '$lib/types';
  import { t } from '$lib/i18n';
  import { getMessagePreview, formatGroupTime } from './chat-utils';

  interface Props {
    pinned: MessageResponse[];
    onJump: (id: string) => void;
    onUnpin: (id: string) => void;
    activeId?: string | null;
  }

  let { pinned, onJump, onUnpin, activeId = null }: Props = $props();

  const MAX_SEG = 5;

  let showPopover = $state(false);
  let popListEl = $state<HTMLDivElement | null>(null);

  // Backend returns pins oldest -> newest; surface the newest first. The shown
  // pin tracks scroll position via activeId; tapping walks up to the next older.
  const order = $derived([...pinned].reverse());
  const len = $derived(order.length);
  const activeIdx = $derived(activeId ? order.findIndex((m) => m.id === activeId) : -1);
  const displayIndex = $derived(activeIdx >= 0 ? activeIdx : 0);
  const current = $derived(len > 0 ? order[displayIndex] : null);

  const segCount = $derived(Math.min(len, MAX_SEG));
  const segs = $derived(Array.from({ length: segCount }, (_, i) => i));
  const activeSeg = $derived(
    len <= 1
      ? 0
      : len <= MAX_SEG
        ? displayIndex
        : Math.round((displayIndex / (len - 1)) * (MAX_SEG - 1))
  );

  function preview(m: MessageResponse): string {
    if (m.fileDetails) return '📎 ' + m.fileDetails.filename;
    return getMessagePreview(m.content);
  }

  // `order` runs newest -> oldest, so stepping forward in it walks back in time.
  function step(delta: number) {
    if (!current) return;
    if (activeIdx < 0) {
      onJump(current.id);
      return;
    }
    onJump(order[(displayIndex + delta + len) % len].id);
  }

  function openPopover() {
    showPopover = true;
    requestAnimationFrame(() => {
      popListEl?.querySelector('.pin-pop-row.current')?.scrollIntoView({ block: 'nearest' });
    });
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && showPopover) {
      showPopover = false;
      e.stopPropagation();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

{#if len > 0 && current}
  <div class="pin-bar">
    <span class="pin-rail" aria-hidden="true">
      {#each segs as i (i)}
        <span class="rail-seg" class:active={i === activeSeg}></span>
      {/each}
    </span>

    <button class="pin-main" onclick={() => step(1)} title={t('chat.pin.jumpTitle')}>
      <Pin size={14} class="pin-icon" />
      <span class="pin-meta">
        <span class="pin-label">{t('chat.pin.pinnedLabel')}</span>
        {#if len > 1}<span class="pin-count">{displayIndex + 1}/{len}</span>{/if}
      </span>
      {#key current.id}
        <span class="pin-preview">{preview(current)}</span>
      {/key}
    </button>

    {#if len > 1}
      <div class="pin-nav">
        <button
          class="pin-step"
          onclick={() => step(1)}
          title={t('chat.pin.older')}
          aria-label={t('chat.pin.older')}
        >
          <ChevronUp size={13} />
        </button>
        <button
          class="pin-step"
          onclick={() => step(-1)}
          title={t('chat.pin.newer')}
          aria-label={t('chat.pin.newer')}
        >
          <ChevronDown size={13} />
        </button>
      </div>
    {/if}

    <button
      class="pin-list-btn"
      class:active={showPopover}
      onclick={() => (showPopover ? (showPopover = false) : openPopover())}
      title={t('chat.pin.showAll')}
      aria-label={t('chat.pin.showAll')}
    >
      <List size={15} />
    </button>

    {#if showPopover}
      <div class="pin-pop-backdrop" onclick={() => (showPopover = false)} role="presentation"></div>
      <div class="pin-pop" role="dialog" aria-label={t('chat.pin.title')}>
        <div class="pin-pop-head">
          <span class="pin-pop-title">{t('chat.pin.title')}</span>
          <span class="pin-pop-count">{len}</span>
        </div>
        <div class="pin-pop-list" bind:this={popListEl}>
          {#each order as m (m.id)}
            <div class="pin-pop-row" class:current={m.id === current.id}>
              <button
                class="pin-pop-jump"
                onclick={() => {
                  onJump(m.id);
                  showPopover = false;
                }}
              >
                <span class="pin-dot" class:sent={m.direction === 'sent'}></span>
                <span class="pin-pop-body">
                  <span class="pin-pop-preview">{preview(m)}</span>
                  <span class="pin-pop-time">{formatGroupTime(m.timestamp)}</span>
                </span>
              </button>
              <button
                class="pin-pop-unpin"
                onclick={() => onUnpin(m.id)}
                title={t('chat.pin.unpin')}
                aria-label={t('chat.pin.unpin')}
              >
                <X size={14} />
              </button>
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
{/if}

<style>
  /* Above the composer (12), the header (10) and the sticky day pills (4):
     backdrop-filter makes this a stacking context, so the popover can only
     rise as high as the bar itself does. */
  .pin-bar {
    position: relative;
    z-index: 14;
    display: flex;
    align-items: stretch;
    gap: 2px;
    padding: 5px max(8px, var(--safe-right)) 5px max(8px, var(--safe-left));
    background: var(--bg-secondary);
    backdrop-filter: blur(14px) saturate(140%);
    -webkit-backdrop-filter: blur(14px) saturate(140%);
    border-bottom: 1px solid var(--border-light);
    flex-shrink: 0;
  }
  /* Translucency only where color-mix() exists: a color-mix() value containing
     var() computes to `transparent` on engines without it (Chrome < 111). */
  @supports (background: color-mix(in srgb, red 50%, transparent)) {
    .pin-bar {
      background: color-mix(in srgb, var(--bg-secondary) 86%, transparent);
    }
  }

  .pin-rail {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 3px;
    align-self: stretch;
    flex-shrink: 0;
    margin-right: 6px;
  }
  .rail-seg {
    flex: 1;
    min-height: 4px;
    border-radius: 2px;
    background: var(--accent-dim);
    transition: background var(--transition);
  }
  .rail-seg.active {
    background: var(--accent);
  }

  .pin-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    text-align: left;
    transition: background var(--transition);
  }
  .pin-main:hover {
    background: var(--bg-hover);
  }
  .pin-main:active {
    transform: scale(0.99);
  }
  .pin-main :global(.pin-icon) {
    color: var(--accent);
    flex-shrink: 0;
  }
  .pin-nav {
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 1px;
    flex-shrink: 0;
  }
  .pin-step {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 15px;
    border-radius: 4px;
    color: var(--text-muted);
    transition: all var(--transition);
  }
  .pin-step:hover {
    background: var(--bg-hover);
    color: var(--accent-hover);
  }
  .pin-step:active {
    transform: scale(0.94);
  }

  .pin-meta {
    display: inline-flex;
    align-items: baseline;
    gap: 6px;
    flex-shrink: 0;
  }
  .pin-label {
    font-size: var(--text-2xs);
    font-weight: 700;
    color: var(--accent-hover);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .pin-count {
    font-family: var(--font-mono);
    font-size: 10.5px;
    font-weight: 600;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
  }

  .pin-preview {
    flex: 1;
    min-width: 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .pin-list-btn {
    flex-shrink: 0;
    width: 32px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    transition: all var(--transition);
  }
  .pin-list-btn:hover,
  .pin-list-btn.active {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .pin-pop-backdrop {
    position: fixed;
    inset: 0;
    z-index: 200;
  }
  .pin-pop {
    position: absolute;
    top: calc(100% + 4px);
    right: max(8px, var(--safe-right));
    width: min(340px, calc(var(--safe-w) - 24px));
    max-height: 340px;
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-3);
    z-index: 201;
    overflow: hidden;
  }
  .pin-pop-head {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    font-size: var(--text-2xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    border-bottom: 1px solid var(--border-light);
  }
  .pin-pop-count {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 999px;
    background: var(--accent-dim);
    color: var(--accent);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .pin-pop-list {
    overflow-y: auto;
    padding: 4px;
  }
  .pin-pop-row {
    display: flex;
    align-items: center;
    gap: 2px;
    border-radius: var(--radius-sm);
  }
  .pin-pop-row:hover {
    background: var(--bg-hover);
  }
  .pin-pop-row.current {
    background: var(--accent-dim);
  }
  .pin-pop-jump {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 8px;
    text-align: left;
  }
  .pin-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-muted);
    flex-shrink: 0;
  }
  .pin-dot.sent {
    background: var(--accent);
  }
  .pin-pop-body {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .pin-pop-preview {
    font-size: var(--text-sm);
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .pin-pop-time {
    font-size: var(--text-2xs);
    color: var(--text-muted);
  }
  .pin-pop-unpin {
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    margin-right: 4px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    opacity: 0.5;
    transition: all var(--transition);
  }
  .pin-pop-row:hover .pin-pop-unpin {
    opacity: 1;
  }
  .pin-pop-unpin:hover {
    background: var(--danger-dim);
    color: var(--danger);
  }

  @media (prefers-reduced-motion: no-preference) {
    .pin-bar {
      animation: pin-bar-in 220ms cubic-bezier(0.2, 0.9, 0.3, 1.1);
    }
    .pin-pop {
      animation: pin-pop-in 140ms cubic-bezier(0.2, 0.9, 0.3, 1.1);
    }
    .pin-preview {
      animation: pin-text-in 180ms ease;
    }
  }
  @keyframes pin-bar-in {
    from {
      opacity: 0;
      transform: translateY(-100%);
    }
  }
  @keyframes pin-pop-in {
    from {
      opacity: 0;
      transform: translateY(-6px) scale(0.98);
    }
  }
  @keyframes pin-text-in {
    from {
      opacity: 0;
      transform: translateX(6px);
    }
  }
</style>
