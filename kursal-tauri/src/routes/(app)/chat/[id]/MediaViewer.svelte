<script lang="ts">
  import { onMount } from 'svelte';
  import { X, FolderOpen, Share, ChevronLeft, ChevronRight, Film, Check } from 'lucide-svelte';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';
  import { t } from '$lib/i18n';
  import { notifyError } from '$lib/utils/errors';
  import { flash } from '$lib/utils/flash.svelte';
  import { exportToDevice } from '$lib/utils/file-transfer-paths';
  import { isMobile } from '$lib/api/window';

  interface MediaItem {
    src: string;
    path: string;
    kind: 'image' | 'video';
    filename: string;
  }

  interface Props {
    src: string;
    path: string;
    kind: 'image' | 'video';
    filename: string;
    items?: MediaItem[];
    index?: number;
    onSelect?: (i: number) => void;
    onClose: () => void;
    onPrev?: () => void;
    onNext?: () => void;
    hasPrev?: boolean;
    hasNext?: boolean;
  }

  let {
    src,
    path,
    kind,
    filename,
    items = [],
    index = -1,
    onSelect = () => {},
    onClose,
    onPrev,
    onNext,
    hasPrev = false,
    hasNext = false,
  }: Props = $props();

  // Keep the active thumbnail centered in the filmstrip as navigation moves.
  let stripEl = $state<HTMLElement | null>(null);
  $effect(() => {
    index;
    const active = stripEl?.querySelector('[data-active="true"]') as HTMLElement | null;
    active?.scrollIntoView({ inline: 'center', block: 'nearest', behavior: 'smooth' });
  });

  // WebKit shares one decoded GIF per URL and freezes the chat copy when this
  // viewer closes; the query marker forces a separate resource.
  const isGif = $derived(/\.gif$/i.test(filename));
  const displaySrc = $derived(isGif ? `${src}${src.includes('?') ? '&' : '?'}mv=1` : src);

  let scale = $state(1);
  let tx = $state(0);
  let ty = $state(0);
  let dragging = false;
  let dragStart = { x: 0, y: 0, tx: 0, ty: 0 };
  // Active touch/pen/mouse pointers, keyed by pointerId - drives pinch-zoom.
  const pointers = new Map<number, { x: number; y: number }>();
  let pinchStart: { dist: number; scale: number } | null = null;

  function reset() {
    scale = 1;
    tx = 0;
    ty = 0;
    pointers.clear();
    pinchStart = null;
  }

  // Reset zoom/pan whenever the displayed media changes.
  $effect(() => {
    src;
    reset();
  });

  function onWheel(e: WheelEvent) {
    if (kind !== 'image') return;
    e.preventDefault();
    const next = Math.min(5, Math.max(1, scale - e.deltaY * 0.0015 * scale));
    if (next === 1) {
      tx = 0;
      ty = 0;
    }
    scale = next;
  }
  function onDblClick() {
    if (kind !== 'image') return;
    if (scale > 1) reset();
    else scale = 2.5;
  }
  function startPan(x: number, y: number) {
    dragging = true;
    dragStart = { x, y, tx, ty };
  }
  function onPointerDown(e: PointerEvent) {
    if (kind !== 'image') return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pointers.size === 2) {
      const [a, b] = [...pointers.values()];
      pinchStart = { dist: Math.hypot(a.x - b.x, a.y - b.y), scale };
      dragging = false;
    } else if (pointers.size === 1 && scale > 1) {
      startPan(e.clientX, e.clientY);
    }
  }
  function onPointerMove(e: PointerEvent) {
    if (!pointers.has(e.pointerId)) return;
    pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
    if (pinchStart && pointers.size === 2) {
      const [a, b] = [...pointers.values()];
      const dist = Math.hypot(a.x - b.x, a.y - b.y);
      const next = Math.min(5, Math.max(1, (pinchStart.scale * dist) / pinchStart.dist));
      if (next === 1) {
        tx = 0;
        ty = 0;
      }
      scale = next;
      return;
    }
    if (dragging) {
      tx = dragStart.tx + (e.clientX - dragStart.x);
      ty = dragStart.ty + (e.clientY - dragStart.y);
    }
  }
  function onPointerUp(e: PointerEvent) {
    pointers.delete(e.pointerId);
    if (pointers.size < 2) pinchStart = null;
    if (pointers.size === 0) {
      dragging = false;
    } else if (pointers.size === 1 && scale > 1) {
      const [p] = [...pointers.values()];
      startPan(p.x, p.y);
    }
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === 'Escape') onClose();
    else if (e.key === 'ArrowLeft' && hasPrev) onPrev?.();
    else if (e.key === 'ArrowRight' && hasNext) onNext?.();
  }

  async function reveal() {
    try {
      await revealItemInDir(path);
    } catch (e) {
      notifyError(e, 'chat.conversation.errorRevealFile');
    }
  }

  const exported = flash();

  async function saveToDevice() {
    try {
      const saved = await exportToDevice(path, filename);
      if (saved) exported.trigger();
    } catch (e) {
      notifyError(e, 'chat.conversation.errorExportFile');
    }
  }

  onMount(() => {
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="viewer"
  role="dialog"
  aria-modal="true"
  aria-label={filename}
  onclick={onClose}
  tabindex="-1"
>
  <div class="topbar" onclick={(e) => e.stopPropagation()} role="presentation">
    <span class="title" title={filename}>{filename}</span>
    <div class="actions">
      {#if isMobile}
        <button
          class="iconbtn"
          class:confirmed={exported.active}
          onclick={saveToDevice}
          title={t('chat.bubble.saveToDevice')}
          aria-label={exported.active ? t('common.savedToDevice') : t('chat.bubble.saveToDevice')}
        >
          {#if exported.active}
            <Check size={16} />
          {:else}
            <Share size={16} />
          {/if}
        </button>
      {:else}
        <button
          class="iconbtn"
          onclick={reveal}
          title={t('chat.bubble.showInFolder')}
          aria-label={t('chat.bubble.showInFolder')}
        >
          <FolderOpen size={16} />
        </button>
      {/if}
      <button
        class="iconbtn"
        onclick={onClose}
        title={t('common.close')}
        aria-label={t('common.close')}
      >
        <X size={18} />
      </button>
    </div>
  </div>

  {#if hasPrev}
    <button
      class="nav-arrow left"
      onclick={(e) => {
        e.stopPropagation();
        onPrev?.();
      }}
      aria-label={t('chat.media.previous')}
    >
      <ChevronLeft size={26} />
    </button>
  {/if}
  {#if hasNext}
    <button
      class="nav-arrow right"
      onclick={(e) => {
        e.stopPropagation();
        onNext?.();
      }}
      aria-label={t('chat.media.next')}
    >
      <ChevronRight size={26} />
    </button>
  {/if}

  <div class="stage" onclick={(e) => e.stopPropagation()} role="presentation">
    {#if kind === 'image'}
      <img
        class="media"
        class:zoomed={scale > 1}
        src={displaySrc}
        alt={filename}
        style="transform: translate({tx}px, {ty}px) scale({scale});"
        onwheel={onWheel}
        ondblclick={onDblClick}
        onpointerdown={onPointerDown}
        onpointermove={onPointerMove}
        onpointerup={onPointerUp}
        onpointercancel={onPointerUp}
        draggable="false"
      />
    {:else}
      <!-- svelte-ignore a11y_media_has_caption -->
      <video class="media" {src} controls autoplay></video>
    {/if}
  </div>

  {#if items.length > 1}
    <div
      class="filmstrip"
      bind:this={stripEl}
      onclick={(e) => e.stopPropagation()}
      role="presentation"
    >
      {#each items as it, i (it.path)}
        <button
          class="thumb"
          class:active={i === index}
          data-active={i === index}
          aria-current={i === index}
          onclick={() => onSelect(i)}
          title={it.filename}
          aria-label={it.filename}
        >
          {#if it.kind === 'image'}
            <img src={it.src} alt={it.filename} loading="lazy" draggable="false" />
          {:else}
            <span class="thumb-vid"><Film size={16} /></span>
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .viewer {
    position: fixed;
    inset: 0;
    z-index: 400;
    background: rgba(0, 0, 0, 0.82);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    display: flex;
    flex-direction: column;
    animation: fadeIn 0.15s ease;
  }
  @keyframes fadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: calc(10px + var(--safe-top)) calc(14px + var(--safe-right)) 10px
      calc(14px + var(--safe-left));
    color: #fff;
    flex-shrink: 0;
    z-index: 2;
  }
  .title {
    font-size: 13px;
    color: rgba(255, 255, 255, 0.85);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 60vw;
  }
  .actions {
    display: flex;
    gap: 6px;
  }
  .iconbtn {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.12s ease;
  }
  .iconbtn:hover {
    background: rgba(255, 255, 255, 0.22);
  }
  .iconbtn.confirmed {
    background: var(--success);
  }

  .nav-arrow {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2;
    transition: background 0.12s ease;
  }
  .nav-arrow:hover {
    background: rgba(255, 255, 255, 0.22);
  }
  .nav-arrow.left {
    left: 16px;
  }
  .nav-arrow.right {
    right: 16px;
  }

  .stage {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 8px 16px 24px;
    overflow: hidden;
  }
  .media {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    border-radius: 8px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.6);
    background: #000;
    will-change: transform;
    user-select: none;
    -webkit-user-select: none;
  }
  img.media {
    touch-action: none;
  }
  img.media.zoomed {
    cursor: grab;
    border-radius: 0;
  }
  img.media.zoomed:active {
    cursor: grabbing;
  }

  .filmstrip {
    flex-shrink: 0;
    display: flex;
    gap: 6px;
    align-items: center;
    justify-content: safe center;
    overflow-x: auto;
    overflow-y: hidden;
    padding: 8px calc(14px + var(--safe-right)) calc(10px + var(--safe-bottom))
      calc(14px + var(--safe-left));
    scrollbar-width: none;
    z-index: 2;
  }
  .filmstrip::-webkit-scrollbar {
    display: none;
  }
  .thumb {
    flex-shrink: 0;
    width: 46px;
    height: 46px;
    border-radius: 8px;
    overflow: hidden;
    padding: 0;
    background: rgba(255, 255, 255, 0.08);
    border: 2px solid transparent;
    opacity: 0.55;
    cursor: pointer;
    transition:
      opacity 0.14s ease,
      border-color 0.14s ease,
      transform 0.14s ease;
  }
  .thumb:hover {
    opacity: 0.85;
  }
  .thumb.active {
    opacity: 1;
    border-color: #fff;
    transform: translateY(-2px);
  }
  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    user-select: none;
    -webkit-user-select: none;
  }
  .thumb-vid {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: rgba(255, 255, 255, 0.7);
  }
</style>
