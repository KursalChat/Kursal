<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { log } from '$lib/utils/log';
  import { t } from '$lib/i18n';
  import { scale } from 'svelte/transition';
  import { ZoomIn, ZoomOut, X, Check } from 'lucide-svelte';
  import Button from './Button.svelte';
  import { notifications } from '$lib/state/notifications.svelte';

  const MAX_AVATAR_BYTES = 200 * 1024;

  let {
    file,
    onConfirm,
    onCancel,
  }: {
    file: Blob;
    onConfirm: (base64: string, bytes: number[]) => void;
    onCancel: () => void;
  } = $props();

  const VIEWPORT = 260;
  const OUTPUT_SIZE = 256;

  let img = $state<HTMLImageElement | null>(null);
  let blobUrl = $state<string | null>(null);
  let naturalW = $state(0);
  let naturalH = $state(0);

  let scaleVal = $state(1);
  let minScale = $state(1);
  let maxScale = $state(4);
  let tx = $state(0);
  let ty = $state(0);

  let dragging = false;
  let dragStartX = 0;
  let dragStartY = 0;
  let dragStartTx = 0;
  let dragStartTy = 0;

  let processing = $state(false);

  onMount(() => {
    blobUrl = URL.createObjectURL(file);
    const im = new Image();
    im.onload = () => {
      naturalW = im.naturalWidth;
      naturalH = im.naturalHeight;
      const m = VIEWPORT / Math.min(naturalW, naturalH);
      minScale = m;
      maxScale = Math.max(m * 4, m + 0.5);
      scaleVal = m;
      tx = (VIEWPORT - naturalW * m) / 2;
      ty = (VIEWPORT - naturalH * m) / 2;
      img = im;
    };
    im.src = blobUrl;
  });

  onDestroy(() => {
    if (blobUrl) URL.revokeObjectURL(blobUrl);
  });

  function clamp() {
    const w = naturalW * scaleVal;
    const h = naturalH * scaleVal;
    if (tx > 0) tx = 0;
    if (ty > 0) ty = 0;
    if (tx < VIEWPORT - w) tx = VIEWPORT - w;
    if (ty < VIEWPORT - h) ty = VIEWPORT - h;
  }

  function setScale(next: number, anchorX = VIEWPORT / 2, anchorY = VIEWPORT / 2) {
    const clamped = Math.min(maxScale, Math.max(minScale, next));
    if (clamped === scaleVal) return;
    const ratio = clamped / scaleVal;
    tx = anchorX - (anchorX - tx) * ratio;
    ty = anchorY - (anchorY - ty) * ratio;
    scaleVal = clamped;
    clamp();
  }

  function onPointerDown(e: PointerEvent) {
    if (!img) return;
    dragging = true;
    dragStartX = e.clientX;
    dragStartY = e.clientY;
    dragStartTx = tx;
    dragStartTy = ty;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging) return;
    tx = dragStartTx + (e.clientX - dragStartX);
    ty = dragStartTy + (e.clientY - dragStartY);
    clamp();
  }

  function onPointerUp(e: PointerEvent) {
    dragging = false;
    try {
      (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
    } catch {}
  }

  function onWheel(e: WheelEvent) {
    if (!img) return;
    e.preventDefault();
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const ax = e.clientX - rect.left;
    const ay = e.clientY - rect.top;
    const factor = Math.exp(-e.deltaY * 0.0015);
    setScale(scaleVal * factor, ax, ay);
  }

  function onSliderInput(e: Event) {
    const v = parseFloat((e.target as HTMLInputElement).value);
    setScale(v);
  }

  async function handleConfirm() {
    if (!img) return;
    processing = true;
    try {
      const canvas = document.createElement('canvas');
      canvas.width = OUTPUT_SIZE;
      canvas.height = OUTPUT_SIZE;
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('No 2D context');
      const sx = -tx / scaleVal;
      const sy = -ty / scaleVal;
      const sw = VIEWPORT / scaleVal;
      const sh = VIEWPORT / scaleVal;
      ctx.drawImage(img, sx, sy, sw, sh, 0, 0, OUTPUT_SIZE, OUTPUT_SIZE);

      let quality = 0.85;
      let dataUrl = canvas.toDataURL('image/webp', quality);
      let b64 = dataUrl.split(',')[1];
      let bytes = Array.from(Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)));
      while (bytes.length > MAX_AVATAR_BYTES && quality > 0.3) {
        quality -= 0.1;
        dataUrl = canvas.toDataURL('image/webp', quality);
        b64 = dataUrl.split(',')[1];
        bytes = Array.from(Uint8Array.from(atob(b64), (c) => c.charCodeAt(0)));
      }
      if (bytes.length > MAX_AVATAR_BYTES) {
        notifications.push(t('avatar.errorTooLarge'), 'error');
        processing = false;
        return;
      }
      onConfirm(b64, bytes);
    } catch (e) {
      log.error('Crop failed', e);
      notifications.push(t('avatar.errorProcess'), 'error');
      processing = false;
    }
  }

  function handleBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget && !processing) onCancel();
  }

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        node.remove();
      },
    };
  }
</script>

<div
  class="backdrop"
  use:portal
  role="presentation"
  onclick={handleBackdrop}
  onkeydown={(e) => {
    if (e.key === 'Escape' && !processing) onCancel();
  }}
>
  <div class="modal" in:scale role="dialog" aria-modal="true" tabindex="-1">
    <div class="head">
      <h2>{t('avatar.cropperTitle')}</h2>
      <button type="button" class="icon-btn" aria-label={t('common.cancel')} onclick={onCancel}>
        <X size={16} />
      </button>
    </div>

    <p class="hint">{t('avatar.cropperHint')}</p>

    <div
      class="viewport"
      onpointerdown={onPointerDown}
      onpointermove={onPointerMove}
      onpointerup={onPointerUp}
      onpointercancel={onPointerUp}
      onwheel={onWheel}
      role="presentation"
    >
      {#if blobUrl}
        <img
          class="crop-img"
          src={blobUrl}
          alt=""
          draggable="false"
          style="width:{naturalW * scaleVal}px;height:{naturalH *
            scaleVal}px;transform:translate({tx}px,{ty}px);"
        />
      {/if}
      <div class="ring"></div>
    </div>

    <div class="zoom">
      <button
        type="button"
        class="icon-btn"
        aria-label={t('avatar.zoomOut')}
        onclick={() => setScale(scaleVal / 1.2)}
      >
        <ZoomOut size={14} />
      </button>
      <input
        type="range"
        min={minScale}
        max={maxScale}
        step={(maxScale - minScale) / 100 || 0.01}
        value={scaleVal}
        oninput={onSliderInput}
        aria-label={t('avatar.zoom')}
      />
      <button
        type="button"
        class="icon-btn"
        aria-label={t('avatar.zoomIn')}
        onclick={() => setScale(scaleVal * 1.2)}
      >
        <ZoomIn size={14} />
      </button>
    </div>

    <div class="actions">
      <Button variant="secondary" onclick={onCancel} disabled={processing}
        >{t('common.cancel')}</Button
      >
      <Button onclick={handleConfirm} loading={processing} disabled={!img}>
        <Check size={13} /> Use photo
      </Button>
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.72);
    backdrop-filter: blur(6px);
    -webkit-backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 9999;
    padding: var(--safe-top) var(--safe-right) var(--safe-bottom) var(--safe-left);
  }
  .modal {
    background: var(--bg-secondary);
    border-radius: var(--radius-lg);
    padding: 20px;
    width: 320px;
    max-width: 92vw;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 4px;
  }
  h2 {
    font-size: 15px;
    font-weight: 600;
    margin: 0;
  }
  .hint {
    font-size: 12px;
    color: var(--text-secondary);
    margin: 0 0 14px;
  }
  .icon-btn {
    width: 28px;
    height: 28px;
    border-radius: var(--radius-md);
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-secondary);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition:
      background var(--transition),
      color var(--transition);
  }
  .icon-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .viewport {
    position: relative;
    width: 260px;
    height: 260px;
    margin: 0 auto;
    border-radius: 50%;
    overflow: hidden;
    background: #000;
    cursor: grab;
    touch-action: none;
    user-select: none;
  }
  .viewport:active {
    cursor: grabbing;
  }
  .crop-img {
    position: absolute;
    top: 0;
    left: 0;
    transform-origin: 0 0;
    pointer-events: none;
    -webkit-user-drag: none;
  }
  .ring {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    box-shadow: inset 0 0 0 2px rgba(255, 255, 255, 0.5);
    pointer-events: none;
  }
  .zoom {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 14px 0 16px;
  }
  .zoom input[type='range'] {
    flex: 1;
    accent-color: var(--accent-solid);
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  :global(.actions .button) {
    min-width: 96px;
  }
</style>
