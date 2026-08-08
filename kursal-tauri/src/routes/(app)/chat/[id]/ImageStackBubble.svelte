<script lang="ts">
  import { Check, CheckCheck, CloudUpload, Download, ImageOff } from 'lucide-svelte';
  import { pathExists } from '$lib/api/fs';
  import Spinner from '$lib/components/Spinner.svelte';
  import type { MessageResponse } from '$lib/types';
  import { t } from '$lib/i18n';
  import { messagesState } from '$lib/state/messages.svelte';
  import {
    formatTime,
    formatFullTimestamp,
    formatFileSize,
    isTransferDone,
    mediaUrl,
  } from './chat-utils';

  interface Props {
    msgs: MessageResponse[];
    direction: 'sent' | 'received';
    onOpen: (msg: MessageResponse, stack: MessageResponse[]) => void;
    onDownloadAll: (msgs: MessageResponse[]) => void;
    onContextMenu: (e: MouseEvent, msg: MessageResponse) => void;
  }

  let { msgs, direction, onOpen, onDownloadAll, onContextMenu }: Props = $props();

  // Show the first three plus the newest, so a just-sent/received image is
  // always visible (in the 4th slot, under the "+N" overlay) rather than buried
  // behind the oldest four.
  const visible = $derived(
    msgs.length > 4 ? [msgs[0], msgs[1], msgs[2], msgs[msgs.length - 1]] : msgs.slice(0, 4)
  );
  const hiddenCount = $derived(Math.max(0, msgs.length - 4));
  const last = $derived(msgs[msgs.length - 1]);

  // "Is the file actually on disk" per image. autodownloadPath can point at a file
  // that never finished downloading or was cleared, which would otherwise render a
  // broken <img>. Only explicit `false` means missing; undefined renders optimistically.
  let fileStatus = $state<Record<string, boolean>>({});
  let lastSig = $state('');
  $effect(() => {
    const snap = msgs.map((m) => ({
      id: m.id,
      path: m.fileDetails?.autodownloadPath ?? null,
      v: messagesState.mediaVersionFor(m.contactId, m.id),
    }));
    const sig = snap.map((s) => `${s.id}:${s.path ?? ''}:${s.v}`).join('|');
    if (sig === lastSig) return;
    lastSig = sig;
    let cancelled = false;
    (async () => {
      const present = await Promise.all(
        snap.map((s) => (s.path ? pathExists(s.path) : Promise.resolve(false)))
      );
      const next: Record<string, boolean> = {};
      snap.forEach((s, i) => (next[s.id] = present[i]));
      if (!cancelled) fileStatus = next;
    })();
    return () => {
      cancelled = true;
    };
  });

  type TileState = 'ready' | 'downloading' | 'missing';
  function tileState(m: MessageResponse): TileState {
    const path = m.fileDetails?.autodownloadPath;
    // Sent images are always local, so their thumbnail shows immediately even
    // while the copy uploads to the peer.
    if (m.direction === 'sent') return path ? 'ready' : 'missing';
    const prog = messagesState.transferProgressFor(m.id);
    if (prog && !isTransferDone(prog)) return 'downloading';
    if (!path) return 'missing';
    return fileStatus[m.id] === false ? 'missing' : 'ready';
  }

  const missingMsgs = $derived(msgs.filter((m) => tileState(m) === 'missing'));
  const missingBytes = $derived(
    missingMsgs.reduce((sum, m) => sum + (m.fileDetails?.sizeBytes ?? 0), 0)
  );
  const missingSizeLabel = $derived(formatFileSize(missingBytes));

  function handleTile(m: MessageResponse) {
    const state = tileState(m);
    if (state === 'ready') onOpen(m, msgs);
    else if (state === 'missing') onDownloadAll(missingMsgs);
  }
</script>

<div
  class="stack"
  class:sent={direction === 'sent'}
  role="group"
  aria-label={t('chat.stack.ariaLabel', { count: msgs.length })}
>
  <div class="grid">
    {#each visible as m, i (m.id)}
      {@const isOverlay = hiddenCount > 0 && i === visible.length - 1}
      {@const state = tileState(m)}
      <button
        class="tile"
        oncontextmenu={(e) => onContextMenu(e, m)}
        onclick={() => handleTile(m)}
        aria-label={isOverlay
          ? t('chat.stack.moreAriaLabel', { count: hiddenCount + 1 })
          : state === 'missing'
            ? t('chat.stack.notDownloaded')
            : m.fileDetails?.filename}
      >
        {#if state === 'downloading'}
          <span class="ph"><Spinner size={18} color="currentColor" /></span>
        {:else if state === 'missing'}
          <span class="ph"><ImageOff size={20} /></span>
        {:else if m.fileDetails?.autodownloadPath}
          <img
            src={mediaUrl(
              m.fileDetails.autodownloadPath,
              messagesState.mediaVersionFor(m.contactId, m.id)
            )}
            alt={m.fileDetails.filename}
            loading="lazy"
          />
        {/if}
        {#if isOverlay}
          <span class="more">+{hiddenCount + 1}</span>
        {/if}
      </button>
    {/each}
  </div>

  {#if missingMsgs.length > 0}
    <button class="dl-all" onclick={() => onDownloadAll(missingMsgs)}>
      <Download size={14} />
      <span>{t('chat.stack.downloadAll')}</span>
      {#if missingSizeLabel}
        <span class="dl-size">{missingSizeLabel}</span>
      {/if}
    </button>
  {/if}

  <div class="meta">
    <span class="time" title={formatFullTimestamp(last.timestamp)}>
      {formatTime(last.timestamp)}
    </span>
    {#if direction === 'sent'}
      {#if last.status === 'queued' || last.status === 'queued_in_dht'}
        <span class="tick" aria-label={t('chat.bubble.statusQueued')}>
          <CloudUpload size={12} />
        </span>
      {:else if last.status === 'delivered'}
        <span class="tick" aria-label={t('chat.bubble.statusSent')}><Check size={12} /></span>
      {:else if last.status === 'offline_delivered'}
        <span class="tick" aria-label={t('chat.bubble.statusSentOffline')}>
          <CheckCheck size={12} />
        </span>
      {:else if last.status === 'read'}
        <span class="tick read" aria-label={t('chat.bubble.statusRead')}>
          <CheckCheck size={12} />
        </span>
      {/if}
    {/if}
  </div>
</div>

<style>
  .stack {
    display: flex;
    flex-direction: column;
    gap: 3px;
    max-width: 264px;
    width: 100%;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 3px;
    border-radius: var(--radius-md);
    overflow: hidden;
  }
  .tile {
    position: relative;
    aspect-ratio: 1;
    padding: 0;
    overflow: hidden;
    cursor: pointer;
    background: var(--bg-tertiary);
    border: none;
  }
  .tile img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    transition: transform var(--transition);
  }
  .tile:hover img {
    transform: scale(1.04);
  }
  .ph {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    background: var(--bg-tertiary);
  }
  .more {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(2, 6, 23, 0.55);
    color: #fff;
    font-size: 20px;
    font-weight: 700;
    backdrop-filter: blur(2px);
    -webkit-backdrop-filter: blur(2px);
  }
  .dl-all {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    align-self: flex-start;
    padding: 6px 10px;
    border-radius: var(--radius-md);
    background: var(--accent);
    color: #fff;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    transition: background var(--transition);
  }
  .stack.sent .dl-all {
    align-self: flex-end;
  }
  .dl-all:hover {
    background: var(--accent-hover);
  }
  .dl-size {
    font-weight: 500;
    font-variant-numeric: tabular-nums;
    opacity: 0.85;
  }
  .dl-size::before {
    content: '·';
    margin-right: 6px;
    opacity: 0.7;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 4px;
    justify-content: flex-start;
    font-size: 10.5px;
    color: var(--text-muted);
    padding: 0 2px;
  }
  .stack.sent .meta {
    justify-content: flex-end;
  }
  .tick {
    display: inline-flex;
    align-items: center;
  }
  .tick.read {
    color: var(--success);
  }
</style>
