<script lang="ts">
  import { readTextFile } from '@tauri-apps/plugin-fs';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';
  import {
    FileText,
    Download,
    Check,
    X,
    Share,
    FolderOpen,
    ChevronDown,
    ChevronUp,
  } from 'lucide-svelte';
  import { pathExists } from '$lib/api/fs';
  import { isMobile } from '$lib/api/window';
  import { notifyError } from '$lib/utils/errors';
  import { formatFileSize } from '$lib/utils/bytes';
  import { midTruncate } from '$lib/utils/text';
  import Spinner from '$lib/components/Spinner.svelte';
  import { t } from '$lib/i18n';
  import { mediaKindFromFilename, isTextFilename, mediaUrl } from './chat-utils';
  import type { MessageResponse } from '$lib/types';

  let {
    msg,
    mediaVersion,
    fileOfferState,
    transferPercent,
    transferInProgress,
    transferDone,
    onAcceptFile,
    onCancelFile,
    onSaveToDevice,
    onOpenMedia,
    onMediaResize,
  }: {
    msg: MessageResponse;
    mediaVersion: number;
    fileOfferState: 'idle' | 'accepting' | 'accepted' | undefined;
    transferPercent: number;
    transferInProgress: boolean;
    transferDone: boolean;
    onAcceptFile: () => void;
    onCancelFile: () => void;
    onSaveToDevice: () => void;
    onOpenMedia: (path: string, kind: 'image' | 'video', filename: string) => void;
    onMediaResize: () => void;
  } = $props();

  const RING_CIRCUMFERENCE = 94.25;
  const TEXT_PREVIEW_MAX_BYTES = 256 * 1024;
  const TEXT_PREVIEW_LINES = 10;

  const localFileReady = $derived(msg.direction === 'sent' || !transferInProgress);

  let mediaLoaded = $state(false);
  let pathMissing = $state(false);
  let textPreview = $state<string | null>(null);
  let textExpandedId = $state<string | null>(null);

  const textExpanded = $derived(textExpandedId === msg.id);
  const textPreviewLong = $derived(
    !!textPreview && textPreview.split('\n').length > TEXT_PREVIEW_LINES
  );

  const autoPath = $derived(
    msg.fileDetails?.autodownloadPath && localFileReady && !pathMissing
      ? msg.fileDetails.autodownloadPath
      : null
  );
  const mediaKind = $derived(
    autoPath && msg.fileDetails ? mediaKindFromFilename(msg.fileDetails.filename) : 'other'
  );
  const mediaSrc = $derived(autoPath ? mediaUrl(autoPath, mediaVersion) : null);

  async function revealLocalFile(path: string) {
    try {
      await revealItemInDir(path);
    } catch (e) {
      notifyError(e, 'chat.bubble.errorRevealFile');
    }
  }

  let lastMediaVersion = $state<number | undefined>(undefined);
  $effect(() => {
    if (mediaVersion === lastMediaVersion) return;
    lastMediaVersion = mediaVersion;
    mediaLoaded = false;
  });

  $effect(() => {
    const path = msg.fileDetails?.autodownloadPath ?? null;
    // Re-runs when the transfer completes: the first check can land before the
    // destination file is created.
    void mediaVersion;
    if (!path || !localFileReady) {
      pathMissing = false;
      return;
    }
    let cancelled = false;
    pathExists(path)
      .then((ok) => {
        if (!cancelled) pathMissing = !ok;
      })
      .catch(() => {
        if (!cancelled) pathMissing = false;
      });
    return () => {
      cancelled = true;
    };
  });

  // Text attachments show their contents inline, clamped like a long message.
  $effect(() => {
    const fd = msg.fileDetails;
    const path = fd?.autodownloadPath ?? null;
    void mediaVersion;
    const eligible =
      !!fd &&
      !!path &&
      !pathMissing &&
      localFileReady &&
      isTextFilename(fd.filename) &&
      fd.sizeBytes <= TEXT_PREVIEW_MAX_BYTES;
    if (!eligible || !path) {
      textPreview = null;
      return;
    }
    let cancelled = false;
    readTextFile(path)
      .then((txt) => {
        if (!cancelled) textPreview = txt;
      })
      .catch(() => {
        if (!cancelled) textPreview = null;
      });
    return () => {
      cancelled = true;
    };
  });
</script>

{#if msg.fileDetails}
  {#if autoPath && mediaSrc && mediaKind === 'image'}
    <button
      class="media-img-btn"
      onclick={() => msg.fileDetails && onOpenMedia(autoPath, 'image', msg.fileDetails.filename)}
    >
      <img
        class="media-img"
        class:loaded={mediaLoaded}
        src={mediaSrc}
        alt={msg.fileDetails.filename}
        loading="lazy"
        onload={() => {
          mediaLoaded = true;
          onMediaResize();
        }}
        onerror={() => (mediaLoaded = true)}
      />
    </button>
  {:else if autoPath && mediaSrc && mediaKind === 'video'}
    <!-- svelte-ignore a11y_media_has_caption -->
    <video
      class="media-video"
      src={mediaSrc}
      controls
      preload="metadata"
      onloadedmetadata={onMediaResize}
      ondblclick={() => msg.fileDetails && onOpenMedia(autoPath, 'video', msg.fileDetails.filename)}
    ></video>
  {:else if autoPath && mediaSrc && mediaKind === 'audio'}
    <audio class="media-audio" src={mediaSrc} controls preload="metadata"></audio>
  {:else if textPreview !== null}
    <div class="text-preview" class:folded={textPreviewLong && !textExpanded}>
      <pre>{textPreview}</pre>
    </div>
    {#if textPreviewLong}
      <button
        class="fold-toggle"
        onclick={(e) => {
          e.stopPropagation();
          textExpandedId = textExpanded ? null : msg.id;
        }}
      >
        {#if textExpanded}
          <ChevronUp size={12} />{t('chat.bubble.showLess')}
        {:else}
          <ChevronDown size={12} />{t('chat.bubble.showMore')}
        {/if}
      </button>
    {/if}
  {/if}

  <div
    class="file-bubble"
    class:sent={msg.direction === 'sent'}
    class:embedded={!!autoPath && (mediaKind !== 'other' || textPreview !== null)}
  >
    <div class="file-icon"><FileText size={22} /></div>
    <div class="file-info">
      <span class="file-name" title={msg.fileDetails.filename}>
        {midTruncate(msg.fileDetails.filename, 30)}
      </span>
      <span class="file-size">
        {formatFileSize(msg.fileDetails.sizeBytes) ||
          (msg.direction === 'sent'
            ? t('chat.bubble.fileSizeSent')
            : t('chat.bubble.fileSizeReceived'))}
      </span>
    </div>
    {#if msg.direction === 'received'}
      {#if autoPath}
        <!-- Downloads land in app storage: desktop can jump to them
             in the file manager, mobile can only copy them out. -->
        {#if isMobile}
          <button
            class="file-dl-btn ghost"
            title={t('chat.bubble.saveToDevice')}
            aria-label={t('chat.bubble.saveToDevice')}
            onclick={onSaveToDevice}
          >
            <Share size={16} />
          </button>
        {:else}
          <button
            class="file-dl-btn ghost"
            title={t('chat.bubble.showInFolder')}
            aria-label={t('chat.bubble.showInFolder')}
            onclick={() => revealLocalFile(autoPath)}
          >
            <FolderOpen size={16} />
          </button>
        {/if}
      {:else if fileOfferState === 'accepted' && transferDone}
        <span class="file-done" title={t('chat.bubble.complete')}>
          <Check size={18} />
        </span>
      {:else if transferInProgress || fileOfferState === 'accepted'}
        <span class="file-progress-ring" aria-label="{transferPercent}%">
          <svg viewBox="0 0 36 36" width="36" height="36">
            <circle cx="18" cy="18" r="15" class="ring-track" />
            <circle
              cx="18"
              cy="18"
              r="15"
              class="ring-fill"
              stroke-dasharray="{(transferPercent / 100) * RING_CIRCUMFERENCE} {RING_CIRCUMFERENCE}"
            />
          </svg>
          <span class="ring-pct">{transferPercent}%</span>
        </span>
        <button
          class="file-dl-btn ghost"
          title={t('chat.bubble.cancel')}
          aria-label={t('chat.bubble.cancel')}
          onclick={onCancelFile}
        >
          <X size={16} />
        </button>
      {:else}
        <button
          class="file-dl-btn"
          title={t('chat.bubble.download')}
          aria-label={t('chat.bubble.downloadAriaLabel')}
          disabled={fileOfferState === 'accepting'}
          onclick={onAcceptFile}
        >
          {#if fileOfferState === 'accepting'}
            <Spinner size={14} color="#fff" />
          {:else}
            <Download size={16} />
          {/if}
        </button>
      {/if}
    {:else if transferInProgress}
      <span class="file-progress-ring" aria-label="{transferPercent}%">
        <svg viewBox="0 0 36 36" width="36" height="36">
          <circle cx="18" cy="18" r="15" class="ring-track" />
          <circle
            cx="18"
            cy="18"
            r="15"
            class="ring-fill on-sent"
            stroke-dasharray="{(transferPercent / 100) * RING_CIRCUMFERENCE} {RING_CIRCUMFERENCE}"
          />
        </svg>
        <span class="ring-pct">{transferPercent}%</span>
      </span>
      <button
        class="file-dl-btn ghost"
        title={t('chat.bubble.cancel')}
        aria-label={t('chat.bubble.cancel')}
        onclick={onCancelFile}
      >
        <X size={16} />
      </button>
    {/if}
  </div>
{/if}

<style>
  .text-preview {
    max-width: min(440px, 72vw);
    max-height: calc(1.45em * 25);
    margin-bottom: 4px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: rgba(0, 0, 0, 0.28);
    overflow-y: auto;
    overscroll-behavior: contain;
  }
  .text-preview.folded {
    max-height: calc(1.45em * 10);
    overflow: hidden;
    -webkit-mask-image: linear-gradient(to bottom, #000 78%, transparent 100%);
    mask-image: linear-gradient(to bottom, #000 78%, transparent 100%);
  }
  .text-preview pre {
    margin: 0;
    padding: 0;
    background: transparent;
    font-family: var(--font-mono);
    font-size: 12.5px;
    line-height: 1.45;
    white-space: pre-wrap;
    word-break: break-word;
  }
  .fold-toggle {
    display: flex;
    align-items: center;
    gap: 3px;
    margin-top: 3px;
    padding: 0;
    width: fit-content;
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--accent);
    background: none;
  }
  @media (hover: hover) {
    .fold-toggle:hover {
      text-decoration: underline;
    }
  }
  .media-img-btn {
    display: block;
    padding: 0;
    background: none;
    border: 0;
    cursor: zoom-in;
    border-radius: var(--radius-sm);
    overflow: hidden;
    line-height: 0;
    max-width: 100%;
  }
  .media-img {
    display: block;
    max-width: 320px;
    max-height: 320px;
    width: auto;
    height: auto;
    object-fit: contain;
    border-radius: var(--radius-sm);
    opacity: 0;
    min-height: 140px;
    min-width: 140px;
    background: linear-gradient(
      100deg,
      var(--bg-hover) 30%,
      color-mix(in srgb, var(--bg-hover) 50%, transparent) 50%,
      var(--bg-hover) 70%
    );
    background-size: 200% 100%;
    animation: media-skeleton 1.3s linear infinite;
    transition: opacity 0.22s ease;
  }
  .media-img.loaded {
    opacity: 1;
    min-height: 0;
    min-width: 0;
    background: none;
    animation: none;
  }
  @keyframes media-skeleton {
    from {
      background-position: 200% 0;
    }
    to {
      background-position: -200% 0;
    }
  }
  .media-video {
    display: block;
    max-width: 320px;
    max-height: 360px;
    width: 100%;
    border-radius: var(--radius-sm);
    background: #000;
  }
  .media-audio {
    display: block;
    width: 280px;
    max-width: 100%;
  }
  .file-bubble {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px 6px 4px;
    min-width: 240px;
    max-width: 340px;
    border-left: 3px solid var(--text-muted);
    padding-left: 10px;
    border-radius: 2px;
  }
  .file-bubble.embedded {
    margin-top: 8px;
    min-width: 0;
    padding-top: 8px;
    border-top: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 0;
    padding-left: 4px;
    border-left: 0;
  }
  .file-bubble.embedded:not(.sent) {
    border-top-color: var(--border-light);
  }
  .file-icon {
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--text-muted) 18%, transparent);
    border-radius: var(--radius-md);
    flex-shrink: 0;
  }
  .file-bubble.sent .file-icon {
    background: rgba(255, 255, 255, 0.18);
    color: #fff;
  }
  .file-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .file-name {
    font-weight: 600;
    font-size: 13.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: ltr;
  }
  .file-size {
    font-size: var(--text-2xs);
    opacity: 0.72;
    font-variant-numeric: tabular-nums;
  }
  .file-dl-btn {
    width: 36px;
    height: 36px;
    border-radius: var(--radius-sm);
    background: var(--accent);
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition: all var(--transition);
    flex-shrink: 0;
  }
  .file-dl-btn.ghost {
    background: var(--bg-hover);
    color: var(--text-secondary);
  }
  .file-bubble.sent .file-dl-btn.ghost {
    background: rgba(255, 255, 255, 0.18);
    color: #fff;
  }
  @media (hover: hover) {
    .file-dl-btn:hover:not(:disabled) {
      background: var(--accent-hover);
      transform: scale(1.05);
    }
    .file-dl-btn.ghost:hover:not(:disabled) {
      background: var(--bg-hover);
    }
  }
  .file-dl-btn:active:not(:disabled) {
    transform: scale(0.95);
  }
  .file-dl-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .file-done {
    width: 36px;
    height: 36px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--success) 22%, transparent);
    color: var(--success);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .file-progress-ring {
    position: relative;
    width: 36px;
    height: 36px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .file-progress-ring svg {
    transform: rotate(-90deg);
    overflow: visible;
  }
  .file-progress-ring .ring-track {
    fill: none;
    stroke: var(--bg-hover);
    stroke-width: 3;
  }
  .file-bubble.sent .file-progress-ring .ring-track {
    stroke: rgba(255, 255, 255, 0.2);
  }
  .file-progress-ring .ring-fill {
    fill: none;
    stroke: var(--accent);
    stroke-width: 3;
    stroke-linecap: round;
    transition: stroke-dasharray 200ms linear;
  }
  .file-progress-ring .ring-fill.on-sent,
  .file-bubble.sent .file-progress-ring .ring-fill {
    stroke: #fff;
  }
  .ring-pct {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 9px;
    font-weight: 700;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }
  .file-bubble.sent .ring-pct {
    color: #fff;
  }
</style>
