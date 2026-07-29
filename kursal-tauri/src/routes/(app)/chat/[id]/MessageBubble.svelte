<script lang="ts">
  import {
    Reply,
    Copy,
    Forward,
    Pencil,
    Pin,
    Trash2,
    RotateCw,
    Smile,
    FileText,
    Download,
    Check,
    CheckCheck,
    CloudUpload,
    CloudDownload,
    CircleAlert,
    FolderOpen,
    Share,
    X,
    Clock,
    Ellipsis,
  } from 'lucide-svelte';
  import { fade } from 'svelte/transition';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';
  import { exists } from '@tauri-apps/plugin-fs';
  import { isMobile } from '$lib/api/window';
  import { t } from '$lib/i18n';
  import Spinner from '$lib/components/Spinner.svelte';
  import { notifyError } from '$lib/utils/errors';
  import type { MessageResponse } from '$lib/types';
  import {
    formatFileSize,
    formatFullTimestamp,
    formatTime,
    receivedHoverLabel,
    getMessagePreview,
    mediaKindFromFilename,
    renderMarkdown,
    highlightTerm,
    midTruncate,
    fileTypeColor,
    mediaUrl,
    isMessageActionable,
  } from './chat-utils';

  async function revealLocalFile(path: string) {
    try {
      await revealItemInDir(path);
    } catch (e) {
      notifyError(e, 'chat.bubble.errorRevealFile');
    }
  }

  interface Reaction {
    emoji: string;
    userIds: string[];
  }

  interface Props {
    msg: MessageResponse;
    repliedMessage: MessageResponse | null;
    reactions: Reaction[];
    syncPending?: boolean;
    syncPendingDelete?: boolean;
    userId: string;
    peerName: string;
    layout: 'bubble' | 'flat';
    isFirst: boolean;
    isLast: boolean;
    animateIn: boolean;
    isCoarsePointer: boolean;
    hovered: boolean;
    emojiOpen: boolean;
    searchTerm?: string;
    swipeDx: number;
    fileOfferState: 'idle' | 'accepting' | 'accepted' | undefined;
    flashed: boolean;
    transferPercent: number;
    transferInProgress: boolean;
    transferDone: boolean;
    mediaVersion: number;
    onHoverEnter: () => void;
    onHoverLeave: () => void;
    onTouchStart: (e: TouchEvent) => void;
    onTouchMove: (e: TouchEvent) => void;
    onTouchEnd: () => void;
    onTouchCancel: () => void;
    onContextMenu: (e: MouseEvent) => void;
    onReplyRefClick: (replyToId: string) => void;
    onAcceptFile: () => void;
    onCancelFile: () => void;
    onSaveToDevice: () => void;
    onToggleReact: (emoji: string) => void;
    onStartReply: () => void;
    onCopy: () => void;
    onStartEdit: () => void;
    onTogglePin: () => void;
    onForward: () => void;
    onDelete: () => void;
    onDeleteLocal: () => void;
    onRetry: () => void;
    onToggleEmojiPicker: (rect: DOMRect | null) => void;
    onOpenMedia: (path: string, kind: 'image' | 'video', filename: string) => void;
  }

  let {
    msg,
    repliedMessage,
    reactions,
    syncPending = false,
    syncPendingDelete = false,
    userId,
    peerName,
    layout,
    isFirst,
    isLast,
    animateIn,
    isCoarsePointer,
    hovered,
    emojiOpen,
    searchTerm = '',
    swipeDx,
    fileOfferState,
    flashed,
    transferPercent,
    transferInProgress,
    transferDone,
    mediaVersion,
    onHoverEnter,
    onHoverLeave,
    onTouchStart,
    onTouchMove,
    onTouchEnd,
    onTouchCancel,
    onContextMenu,
    onReplyRefClick,
    onAcceptFile,
    onCancelFile,
    onSaveToDevice,
    onToggleReact,
    onStartReply,
    onCopy,
    onStartEdit,
    onTogglePin,
    onForward,
    onDelete,
    onDeleteLocal,
    onRetry,
    onToggleEmojiPicker,
    onOpenMedia,
  }: Props = $props();

  let mediaLoaded = $state(false);
  let pathMissing = $state(false);
  let lastMediaVersion = $state<typeof mediaVersion | undefined>(undefined);
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
    if (!path || transferInProgress) {
      pathMissing = false;
      return;
    }
    let cancelled = false;
    exists(path)
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

  let menuOpen = $state(false);
  let menuUp = $state(false);
  let menuStyle = $state('');
  let menuEl = $state<HTMLElement | null>(null);
  let moreBtn = $state<HTMLElement | null>(null);
  let rowEl = $state<HTMLElement | null>(null);

  // In-flight and undelivered messages have nothing actionable yet.
  const actionsAvailable = $derived(isMessageActionable(msg.status));

  const MENU_W = 180;
  const MENU_H = 290;

  // Anchors by `right` so it lines up with the menu's top-right transform-origin.
  // Drops below `bottom`, or flips above `top` when there isn't room underneath.
  function positionMenu(left: number, bottom: number, top: number = bottom) {
    const margin = 8;
    const clampedLeft = Math.max(margin, Math.min(left, window.innerWidth - MENU_W - margin));
    const right = Math.round(window.innerWidth - clampedLeft - MENU_W);
    menuUp = window.innerHeight - bottom < MENU_H;
    menuStyle = menuUp
      ? `right:${right}px; bottom:${Math.round(window.innerHeight - top + 6)}px; top:auto;`
      : `right:${right}px; top:${Math.round(bottom + 6)}px;`;
  }

  function toggleMenu(e: MouseEvent) {
    e.stopPropagation();
    if (!menuOpen && moreBtn) {
      const rect = moreBtn.getBoundingClientRect();
      positionMenu(rect.right - MENU_W, rect.bottom, rect.top);
    }
    menuOpen = !menuOpen;
  }

  // The action sheet is the touch affordance. A mouse right-click gets the same
  // menu the "..." button opens, placed at the cursor.
  function handleContextMenu(e: MouseEvent) {
    if (isCoarsePointer) {
      onContextMenu(e);
      return;
    }
    if (!actionsAvailable) return;
    e.preventDefault();
    clearRowSelection();
    positionMenu(e.clientX, e.clientY);
    menuOpen = true;
  }

  // Fallback for engines that select before the press default is dropped.
  function clearRowSelection() {
    const sel = window.getSelection();
    if (!sel || sel.isCollapsed) return;
    if (sel.anchorNode && rowEl?.contains(sel.anchorNode)) sel.removeAllRanges();
  }

  // WebKit selects the word under the cursor on right press; the contextmenu
  // event fires too late to undo it, so the press default is dropped instead.
  function suppressRightPress(e: MouseEvent) {
    if (e.button === 2 || (e.button === 0 && e.ctrlKey)) e.preventDefault();
  }

  function runAction(fn: () => void) {
    fn();
    menuOpen = false;
  }

  // Menu items act on press
  function activate(fn: () => void) {
    return {
      onpointerdown: (e: PointerEvent) => {
        if (e.pointerType !== 'mouse' || e.button !== 0) return;
        e.preventDefault();
        e.stopPropagation();
        runAction(fn);
      },
      onclick: (e: MouseEvent) => {
        e.stopPropagation();
        runAction(fn);
      },
    };
  }

  // The menu mirrors the toolbar's quick actions, so "React" has to hand the
  // picker an anchor - the "..." button, since the menu itself is closing.
  function openPickerFromMenu() {
    const rect = moreBtn?.getBoundingClientRect() ?? null;
    onToggleEmojiPicker(rect);
  }

  function portal(node: HTMLElement) {
    document.body.appendChild(node);
    return {
      destroy() {
        node.remove();
      },
    };
  }

  $effect(() => {
    if (!menuOpen) return;
    function onPointer(e: PointerEvent) {
      const target = e.target as Node;
      if (menuEl?.contains(target) || moreBtn?.contains(target)) return;
      menuOpen = false;
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') menuOpen = false;
    }
    function onReflow() {
      menuOpen = false;
    }
    window.addEventListener('pointerdown', onPointer, true);
    window.addEventListener('keydown', onKey);
    window.addEventListener('scroll', onReflow, true);
    window.addEventListener('resize', onReflow);
    return () => {
      window.removeEventListener('pointerdown', onPointer, true);
      window.removeEventListener('keydown', onKey);
      window.removeEventListener('scroll', onReflow, true);
      window.removeEventListener('resize', onReflow);
    };
  });

  function reactedByLabel(userIds: string[]): string {
    return userIds.map((id) => (id === userId ? t('chat.conversation.you') : peerName)).join(', ');
  }

  // Bounds the chip row so a peer spamming distinct emoji can't grow the message
  // unboundedly. Display-only - the queued reactions still exist in the backend.
  const MAX_VISIBLE_REACTIONS = 20;
  const visibleReactions = $derived(reactions.slice(0, MAX_VISIBLE_REACTIONS));
  const hiddenReactionCount = $derived(Math.max(0, reactions.length - MAX_VISIBLE_REACTIONS));

  const DOUBLE_TAP_REACTION = '❤️';
  let lastTap = 0;

  function canReact() {
    return msg.status !== 'sending' && msg.status !== 'failed' && msg.status !== 'queued';
  }

  function onBubbleDblClick() {
    if (msg.fileDetails) return;
    if (canReact()) onToggleReact(DOUBLE_TAP_REACTION);
  }

  function onBubbleTouchEndReact() {
    if (msg.fileDetails) return;
    const now = Date.now();
    if (now - lastTap < 300 && canReact()) {
      onToggleReact(DOUBLE_TAP_REACTION);
      lastTap = 0;
    } else {
      lastTap = now;
    }
  }

  // Sent messages in a non-terminal state (sending / queued / failed) always
  // show their meta row so the lifecycle icon is visible - independent of the
  // hover/last logic that controls it for delivered messages.
  const showMetaAlways = $derived(
    msg.direction === 'sent' &&
      (msg.status === 'sending' ||
        msg.status === 'queued' ||
        msg.status === 'queued_in_dht' ||
        msg.status === 'failed')
  );
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="msg-row"
  bind:this={rowEl}
  class:first={isFirst}
  class:last={isLast}
  class:hovered
  class:sent={msg.direction === 'sent'}
  class:received={msg.direction === 'received'}
  role="group"
  data-layout={layout}
  data-msg-id={msg.id}
  onmouseenter={onHoverEnter}
  onmouseleave={onHoverLeave}
  onfocusin={onHoverEnter}
  onfocusout={(e) => {
    if (!(e.currentTarget as HTMLElement).contains(e.relatedTarget as Node)) onHoverLeave();
  }}
  ontouchstart={onTouchStart}
  ontouchmove={onTouchMove}
  ontouchend={onTouchEnd}
  ontouchcancel={onTouchCancel}
  onmousedown={suppressRightPress}
  oncontextmenu={handleContextMenu}
>
  {#if swipeDx !== 0}
    <div class="swipe-reply-indicator" style="opacity: {Math.min(Math.abs(swipeDx) / 60, 1)}">
      <Reply size={16} />
    </div>
  {/if}

  {#if msg.replyTo}
    <button
      class="reply-ref"
      class:sent={msg.direction === 'sent'}
      onclick={() => msg.replyTo && onReplyRefClick(msg.replyTo)}
      style="transform: translateX({swipeDx}px); transition: {swipeDx !== 0
        ? 'none'
        : 'transform 0.18s ease'};"
    >
      <span class="reply-label">{t('chat.bubble.replyLabel')}</span>
      <span class="reply-text">
        {repliedMessage
          ? getMessagePreview(repliedMessage.content)
          : t('chat.bubble.replyFallback')}
      </span>
    </button>
  {/if}

  <div class="bubble-row" class:sent={msg.direction === 'sent'}>
    <div class="bubble-anchor" class:sent={msg.direction === 'sent'}>
      <div
        class="bubble"
        class:animate-in={animateIn}
        class:sent={msg.direction === 'sent'}
        class:has-file={!!msg.fileDetails}
        class:failed={msg.status === 'failed'}
        class:queued={msg.status === 'queued' || msg.status === 'queued_in_dht'}
        class:in-dht={msg.status === 'queued_in_dht'}
        class:sending={msg.status === 'sending'}
        class:has-reply={!!msg.replyTo}
        class:sync-deleting={syncPendingDelete}
        role="button"
        tabindex="0"
        style="transform: translateX({swipeDx}px); transition: {swipeDx !== 0
          ? 'none'
          : 'transform 0.18s ease, background 0.15s ease'};"
        ondblclick={onBubbleDblClick}
        onkeydown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            onBubbleDblClick();
          }
        }}
        ontouchend={onBubbleTouchEndReact}
      >
        {#if syncPending}
          <span
            class="sync-flag"
            class:deleting={syncPendingDelete}
            title={syncPendingDelete
              ? t('chat.bubble.pendingDeleteSync')
              : t('chat.bubble.pendingSync')}
          >
            <Clock size={10} />
          </span>
        {/if}
        {#if msg.pinned}
          <span class="pin-flag" title={t('chat.bubble.pinnedLabel')}><Pin size={10} /></span>
        {/if}
        <div class="msg-content" class:selectable={!isCoarsePointer}>
          {#if msg.fileDetails}
            {@const autoPath =
              msg.fileDetails.autodownloadPath && !transferInProgress && !pathMissing
                ? msg.fileDetails.autodownloadPath
                : null}
            {@const mediaKind = autoPath
              ? mediaKindFromFilename(msg.fileDetails.filename)
              : 'other'}
            {@const mediaSrc = autoPath ? mediaUrl(autoPath, mediaVersion) : null}

            {#if autoPath && mediaSrc && mediaKind === 'image'}
              <button
                class="media-img-btn"
                onclick={() =>
                  msg.fileDetails && onOpenMedia(autoPath, 'image', msg.fileDetails.filename)}
              >
                <img
                  class="media-img"
                  class:loaded={mediaLoaded}
                  src={mediaSrc}
                  alt={msg.fileDetails.filename}
                  loading="lazy"
                  onload={() => (mediaLoaded = true)}
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
                ondblclick={() =>
                  msg.fileDetails && onOpenMedia(autoPath, 'video', msg.fileDetails.filename)}
              ></video>
            {:else if autoPath && mediaSrc && mediaKind === 'audio'}
              <audio class="media-audio" src={mediaSrc} controls preload="metadata"></audio>
            {/if}

            <div
              class="file-bubble"
              class:embedded={!!autoPath && mediaKind !== 'other'}
              style="--file-color: {fileTypeColor(msg.fileDetails.filename)};"
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
                        stroke-dasharray="{(transferPercent / 100) * 94.25} 94.25"
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
                      stroke-dasharray="{(transferPercent / 100) * 94.25} 94.25"
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
          {:else}
            {@html searchTerm
              ? highlightTerm(renderMarkdown(msg.content, msg.edited), searchTerm)
              : renderMarkdown(msg.content, msg.edited)}
          {/if}
        </div>

        {#if msg.direction === 'received' && msg.viaOffline}
          <span class="offline-recv" title={t('chat.bubble.deliveredOfflineHint')}>
            <CloudDownload size={11} />
            {t('chat.bubble.deliveredOffline')}
          </span>
        {/if}
      </div>

      {#if reactions.length > 0}
        <div class="reactions" class:sent={msg.direction === 'sent'}>
          {#each visibleReactions as reaction (reaction.emoji)}
            <button
              class="reaction-chip"
              class:mine={reaction.userIds.includes(userId)}
              disabled={!canReact()}
              aria-pressed={reaction.userIds.includes(userId)}
              title={reactedByLabel(reaction.userIds)}
              aria-label={t('chat.bubble.reactionLabel', {
                emoji: reaction.emoji,
                count: reaction.userIds.length,
              })}
              onclick={() => onToggleReact(reaction.emoji)}
            >
              <span class="rem">{reaction.emoji}</span>
              {#if reaction.userIds.length > 1}
                <span class="rcount">{reaction.userIds.length}</span>
              {/if}
            </button>
          {/each}
          {#if hiddenReactionCount > 0}
            <span
              class="reaction-chip overflow"
              title={t('chat.bubble.reactionOverflow', { count: hiddenReactionCount })}
            >
              <span class="rcount">+{hiddenReactionCount}</span>
            </span>
          {/if}
        </div>
      {/if}

      {#if isLast || showMetaAlways || layout === 'flat'}
        {@const receivedLabel = receivedHoverLabel(msg)}
        <div class="msg-meta" class:always-visible={showMetaAlways}>
          <span
            class="msg-time"
            title={receivedLabel
              ? `${formatFullTimestamp(msg.timestamp)} · ${t('chat.bubble.receivedAtLabel', { time: receivedLabel })}`
              : formatFullTimestamp(msg.timestamp)}>{formatTime(msg.timestamp)}</span
          >
          {#if msg.direction === 'sent'}
            {#if msg.status === 'sending'}
              <span class="msg-status sending" aria-label={t('chat.bubble.statusSending')}>
                <Spinner size={11} color="currentColor" />
              </span>
            {:else if msg.status === 'queued'}
              <span class="msg-status queued" aria-label={t('chat.bubble.statusQueued')}>
                <CloudUpload size={12} />
              </span>
            {:else if msg.status === 'queued_in_dht'}
              <span class="msg-status queued in-dht" aria-label={t('chat.bubble.statusUploaded')}>
                <CloudUpload size={12} />
              </span>
            {:else if msg.status === 'delivered'}
              <span class="msg-status delivered" aria-label={t('chat.bubble.statusSent')}>
                <Check size={12} />
              </span>
            {:else if msg.status === 'offline_delivered'}
              <span
                class="msg-status offline-delivered"
                aria-label={t('chat.bubble.statusSentOffline')}
              >
                <CheckCheck size={12} />
              </span>
            {:else if msg.status === 'read'}
              <span class="msg-status read" aria-label={t('chat.bubble.statusRead')}>
                <CheckCheck size={12} />
              </span>
            {:else if msg.status === 'failed'}
              <span
                class="msg-status failed"
                aria-label={t('chat.bubble.statusFailed')}
                title={t('chat.bubble.failedHint')}
              >
                <CircleAlert size={12} />
              </span>
            {/if}
          {/if}
        </div>
      {/if}
      {#if !isCoarsePointer && (hovered || emojiOpen || menuOpen) && actionsAvailable}
        <div class="msg-actions" class:sent={msg.direction === 'sent'}>
          <button
            class="act-btn"
            class:active={emojiOpen}
            title={t('chat.bubble.actionReact')}
            aria-label={t('chat.bubble.actionReact')}
            onmousedown={(e) => e.stopPropagation()}
            onclick={(e) =>
              onToggleEmojiPicker((e.currentTarget as HTMLElement).getBoundingClientRect())}
          >
            <Smile size={15} />
          </button>
          <button
            class="act-btn"
            title={t('chat.bubble.actionReply')}
            aria-label={t('chat.bubble.actionReply')}
            onclick={onStartReply}
          >
            <Reply size={15} />
          </button>
          <div class="act-more">
            <button
              bind:this={moreBtn}
              class="act-btn"
              class:active={menuOpen}
              title={t('chat.bubble.actionMore')}
              aria-label={t('chat.bubble.actionMore')}
              aria-haspopup="menu"
              aria-expanded={menuOpen}
              onmousedown={(e) => e.stopPropagation()}
              onclick={toggleMenu}
            >
              <Ellipsis size={15} />
            </button>
            {#if menuOpen}
              <div
                class="act-menu"
                class:sent={msg.direction === 'sent'}
                class:up={menuUp}
                style={menuStyle}
                use:portal
                bind:this={menuEl}
                role="menu"
              >
                <button class="menu-item" role="menuitem" {...activate(openPickerFromMenu)}>
                  <Smile size={14} />
                  <span>{t('chat.bubble.actionReact')}</span>
                </button>
                <button class="menu-item" role="menuitem" {...activate(onStartReply)}>
                  <Reply size={14} />
                  <span>{t('chat.bubble.actionReply')}</span>
                </button>
                <button class="menu-item" role="menuitem" {...activate(onCopy)}>
                  <Copy size={14} />
                  <span>{t('chat.bubble.actionCopy')}</span>
                </button>
                <button class="menu-item" role="menuitem" {...activate(onTogglePin)}>
                  <Pin size={14} />
                  <span
                    >{msg.pinned ? t('chat.bubble.actionUnpin') : t('chat.bubble.actionPin')}</span
                  >
                </button>
                {#if !msg.fileDetails}
                  <button class="menu-item" role="menuitem" {...activate(onForward)}>
                    <Forward size={14} />
                    <span>{t('chat.bubble.actionForward')}</span>
                  </button>
                {/if}
                {#if msg.direction === 'sent' && !msg.fileDetails && msg.status !== 'queued_in_dht'}
                  <button class="menu-item" role="menuitem" {...activate(onStartEdit)}>
                    <Pencil size={14} />
                    <span>{t('chat.bubble.actionEdit')}</span>
                  </button>
                {/if}
                {#if msg.direction === 'sent'}
                  <button class="menu-item danger" role="menuitem" {...activate(onDelete)}>
                    <Trash2 size={14} />
                    <span>{t('chat.bubble.actionDelete')}</span>
                  </button>
                {/if}
              </div>
            {/if}
          </div>
        </div>
      {/if}
    </div>
    {#if msg.direction === 'sent' && (msg.status === 'queued' || msg.status === 'failed') && (isCoarsePointer || hovered || menuOpen)}
      <div
        class="inline-actions"
        role="group"
        ontouchstart={(e) => e.stopPropagation()}
        ontouchmove={(e) => e.stopPropagation()}
        ontouchend={(e) => e.stopPropagation()}
        onpointerdown={(e) => e.stopPropagation()}
      >
        {#if msg.status === 'failed'}
          <button
            class="inline-action retry"
            onclick={(e) => {
              e.stopPropagation();
              onRetry();
            }}
            title={t('chat.bubble.actionRetry')}
            aria-label={t('chat.bubble.actionRetry')}
          >
            <RotateCw size={13} />
          </button>
        {/if}
        <button
          class="inline-action discard"
          class:queued={msg.status === 'queued'}
          onclick={(e) => {
            e.stopPropagation();
            onDeleteLocal();
          }}
          title={t('chat.bubble.actionDelete')}
          aria-label={t('chat.bubble.actionDelete')}
        >
          <Trash2 size={13} />
        </button>
      </div>
    {/if}
    {#if flashed}
      <span
        class="action-confirm"
        class:sent={msg.direction === 'sent'}
        role="status"
        transition:fade={{ duration: 120 }}
      >
        <Check size={13} />
      </span>
    {/if}
  </div>
</div>

<style>
  .msg-row {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    transition: background 0.3s ease;
    border-radius: var(--radius-md);
    padding: 1px 0;
    touch-action: pan-y;
  }
  .msg-row.sent {
    align-items: flex-end;
  }
  :global(.msg-row.flash) {
    background: var(--accent-dim);
  }

  /* Confirms an action sheet command (copy, save to device) that closed its own
     menu. Anchored to .msg-row on the side away from the bubble tail so it never
     covers text and never reflows the list. */
  .action-confirm {
    position: absolute;
    top: 50%;
    right: 8px;
    transform: translateY(-50%);
    z-index: 100;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--surface);
    border: 1px solid var(--border-light);
    color: var(--success);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.14);
  }
  .action-confirm.sent {
    right: auto;
    left: 8px;
  }

  .swipe-reply-indicator {
    position: absolute;
    left: 4px;
    top: 50%;
    transform: translateY(-50%);
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    background: var(--accent);
    color: #fff;
    display: flex;
    align-items: center;
    justify-content: center;
    pointer-events: none;
    z-index: 2;
  }
  .msg-row.sent .swipe-reply-indicator {
    left: auto;
    right: 4px;
  }

  .bubble {
    --r-big: var(--radius-md);
    --r-sm: var(--radius-sm);
    background: var(--bubble-bg);
    color: var(--text-primary);
    padding: 9px 14px;
    font-size: 14.5px;
    line-height: 1.55;
    width: fit-content;
    max-width: 100%;
    word-break: break-word;
    display: flex;
    flex-direction: column;
    gap: 3px;
    position: relative;
    transition:
      transform 0.18s ease,
      background 0.15s ease,
      box-shadow 0.18s ease;
    border-radius: var(--r-big);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.04);
  }
  .bubble.animate-in {
    animation: bubble-in 220ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  :global([data-theme='light']) .bubble {
    box-shadow: 0 1px 2px rgba(15, 23, 42, 0.05);
  }
  @keyframes bubble-in {
    from {
      transform: scale(0.94) translateY(4px);
      opacity: 0;
    }
    to {
      transform: scale(1) translateY(0);
      opacity: 1;
    }
  }

  /* Grouping pinch: only between adjacent same-author bubbles */
  .msg-row.received:not(.first) .bubble {
    border-top-left-radius: var(--r-sm);
  }
  .msg-row.received:not(.last) .bubble {
    border-bottom-left-radius: var(--r-sm);
  }
  .msg-row.sent:not(.first) .bubble {
    border-top-right-radius: var(--r-sm);
  }
  .msg-row.sent:not(.last) .bubble {
    border-bottom-right-radius: var(--r-sm);
  }

  .bubble.sent {
    background: var(--accent);
    color: #fff;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
  }

  .pin-flag {
    position: absolute;
    top: -7px;
    right: -6px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--bg-secondary);
    color: var(--accent);
    border: 1px solid var(--accent-dim);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.35);
    z-index: 3;
  }
  .msg-row.received .pin-flag {
    right: auto;
    left: -6px;
  }
  @media (prefers-reduced-motion: no-preference) {
    .pin-flag {
      animation: pin-flag-in 200ms cubic-bezier(0.22, 1, 0.36, 1);
    }
  }
  @keyframes pin-flag-in {
    from {
      opacity: 0;
      transform: scale(0.4);
    }
  }

  .sync-flag {
    position: absolute;
    top: -7px;
    left: -6px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: var(--bg-secondary);
    color: var(--text-muted);
    border: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.35);
    z-index: 3;
  }
  .msg-row.received .sync-flag {
    left: auto;
    right: -6px;
  }
  .sync-flag.deleting {
    color: var(--danger);
  }
  .bubble.sync-deleting {
    opacity: 0.55;
  }
  .bubble.sync-deleting .msg-content {
    text-decoration: line-through;
    text-decoration-color: var(--text-muted);
  }

  .bubble.sending {
    opacity: 0.78;
  }
  .bubble.sending::after {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: inherit;
    background: linear-gradient(90deg, transparent, rgba(255, 255, 255, 0.12), transparent);
    background-size: 200% 100%;
    animation: shimmer 1.4s linear infinite;
    pointer-events: none;
  }
  @keyframes shimmer {
    from {
      background-position: 200% 0;
    }
    to {
      background-position: -200% 0;
    }
  }
  .bubble.failed {
    background: var(--danger);
    color: #fff;
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.06);
  }
  .bubble.failed.animate-in {
    animation:
      bubble-in 220ms cubic-bezier(0.22, 1, 0.36, 1),
      failed-pulse 700ms ease-out 220ms 1;
  }
  @keyframes failed-pulse {
    0% {
      box-shadow:
        0 1px 2px rgba(0, 0, 0, 0.06),
        0 0 0 0 color-mix(in srgb, var(--danger) 50%, transparent);
    }
    100% {
      box-shadow:
        0 1px 2px rgba(0, 0, 0, 0.06),
        0 0 0 10px color-mix(in srgb, var(--danger) 0%, transparent);
    }
  }
  .bubble.queued {
    background: var(--accent);
    color: #fff;
    outline: 1.5px dashed var(--accent);
    outline-offset: -1.5px;
    animation: queued-breathe 2.6s ease-in-out infinite;
  }
  .bubble.queued.animate-in {
    animation:
      bubble-in 220ms cubic-bezier(0.22, 1, 0.36, 1),
      queued-breathe 2.6s ease-in-out 220ms infinite;
  }
  .bubble.queued.in-dht {
    background: var(--accent);
    outline-style: solid;
    animation: none;
  }
  @supports (background: color-mix(in srgb, red 50%, transparent)) {
    .bubble.queued {
      background: color-mix(in srgb, var(--accent) 60%, transparent);
      outline-color: color-mix(in srgb, var(--accent) 70%, transparent);
    }
    .bubble.queued.in-dht {
      background: color-mix(in srgb, var(--accent) 80%, transparent);
    }
  }
  .bubble.queued.in-dht.animate-in {
    animation: bubble-in 220ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .msg-status.queued.in-dht {
    animation: none;
    opacity: 1;
  }
  @keyframes queued-breathe {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.78;
    }
  }
  .bubble.has-file {
    padding: 8px;
  }
  .msg-content :global(p) {
    margin: 0;
  }
  .msg-content :global(mark) {
    background: rgba(255, 213, 74, 0.9);
    color: #1a1300;
    border-radius: 2px;
    padding: 0 1px;
  }
  .msg-content :global(p + p) {
    margin-top: 0.45em;
  }
  .msg-content :global(code) {
    font-family: var(--font-mono);
    font-size: 0.88em;
    background: rgba(0, 0, 0, 0.28);
    border-radius: 2px;
    padding: 0.1em 0.35em;
  }
  .msg-content :global(pre) {
    margin: 0.5em 0 0.1em;
    padding: 0.6em 0.7em;
    border-radius: var(--radius-sm);
    background: rgba(0, 0, 0, 0.35);
    overflow-x: auto;
    font-size: 12.5px;
  }
  .msg-content :global(pre code) {
    background: transparent;
    padding: 0;
  }
  .msg-content :global(ul),
  .msg-content :global(ol) {
    margin: 0.4em 0 0;
    padding-left: 1.3em;
  }
  .msg-content :global(li) {
    margin: 0.15em 0;
  }
  .msg-content :global(a) {
    color: inherit;
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .bubble:not(.sent) .msg-content :global(a) {
    color: var(--accent-hover);
  }
  .msg-content :global(blockquote) {
    border-left: 3px solid rgba(255, 255, 255, 0.3);
    padding-left: 8px;
    margin: 0.3em 0;
    color: rgba(255, 255, 255, 0.75);
  }
  .msg-content :global(.edited-tag) {
    font-size: 0.78em;
    opacity: 0.6;
    margin-left: 4px;
    font-style: italic;
  }
  .msg-content :global(.spoiler) {
    background: rgba(128, 128, 128, 0.55);
    color: transparent;
    border-radius: 2px;
    padding: 0 4px;
    cursor: pointer;
    user-select: none;
    transition:
      background 0.18s ease,
      color 0.18s ease;
  }
  .msg-content :global(.spoiler:hover:not(.revealed)) {
    background: rgba(128, 128, 128, 0.7);
  }
  .msg-content :global(.spoiler.revealed) {
    background: color-mix(in srgb, currentColor 14%, transparent);
    color: inherit;
    user-select: text;
  }
  .msg-content :global(.spoiler :is(a, code, strong, em)) {
    color: inherit;
  }
  .msg-content :global(.spoiler.revealed :is(a, code, strong, em)) {
    color: inherit;
  }
  .msg-content :global(.jumbo-emoji) {
    font-size: 2.6em;
    line-height: 1.1;
  }
  .msg-content :global(.code-wrap) {
    position: relative;
  }
  .msg-content :global(.code-copy) {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 24px;
    height: 24px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.35);
    color: #fff;
    opacity: 0;
    transition: opacity 0.15s;
    font-size: 13px;
    cursor: pointer;
    border: none;
    padding: 0;
  }
  .msg-content :global(.code-wrap:hover .code-copy) {
    opacity: 1;
  }
  .msg-content :global(.code-copy.copied) {
    background: var(--success);
  }

  .reply-ref {
    display: flex;
    flex-direction: column;
    gap: 1px;
    font-size: 12px;
    padding: 6px 12px 14px;
    margin-bottom: -10px;
    background: var(--bg-hover);
    border-radius: var(--radius-sm);
    text-align: left;
    cursor: pointer;
    transition: background 140ms ease;
    max-width: min(70%, 420px);
    align-self: flex-start;
    color: var(--text-secondary);
    position: relative;
    z-index: 1;
  }
  .reply-ref.sent {
    align-self: flex-end;
    border-radius: var(--radius-sm);
  }
  .reply-ref:hover {
    background: color-mix(in srgb, var(--text-primary) 12%, transparent);
  }
  .reply-label {
    font-weight: 600;
    font-size: 11px;
    color: var(--accent-hover);
    line-height: 1.2;
  }
  .reply-text {
    opacity: 0.85;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
    line-height: 1.3;
  }
  .bubble.has-reply {
    position: relative;
    z-index: 2;
  }

  /* In flow under the bubble - the .bubble-anchor gap spaces it. Chips used to
     be pulled up over the bubble's edge, which landed them on the timestamp. */
  .reactions {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    max-width: 100%;
    padding: 0 2px;
  }
  .reactions.sent {
    justify-content: flex-end;
  }
  .reaction-chip {
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 2px 7px;
    min-height: 22px;
    box-shadow: 0 1px 3px rgb(0 0 0 / 0.18);
    cursor: pointer;
    transition: transform 180ms cubic-bezier(0.34, 1.56, 0.64, 1);
    user-select: none;
    display: inline-flex;
    align-items: center;
    gap: 3px;
    line-height: 1.2;
    animation: reaction-pop 220ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .reaction-chip:disabled {
    cursor: default;
    opacity: 0.55;
  }
  .reaction-chip:disabled:hover {
    transform: none;
  }
  .reaction-chip.overflow {
    cursor: default;
  }
  @keyframes reaction-pop {
    from {
      transform: scale(0.7);
      opacity: 0;
    }
    to {
      transform: scale(1);
      opacity: 1;
    }
  }
  .reaction-chip:hover {
    transform: scale(1.15);
  }
  @media (prefers-reduced-motion: reduce) {
    .reaction-chip {
      animation: none;
      transition: none;
    }
    .reaction-chip:hover {
      transform: none;
    }
  }
  .reaction-chip.mine {
    background: var(--accent-dim);
    border-color: var(--accent);
  }
  .reaction-chip.mine .rcount {
    color: var(--accent);
    font-weight: 700;
  }
  .rem {
    font-size: 15px;
    line-height: 1;
  }
  .rcount {
    font-size: 12px;
    color: var(--text-muted);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  /* Time + status on their own line under the bubble (see .bubble-anchor), so
     showing them never reflows the bubble itself. */
  .msg-meta {
    display: none;
    align-items: center;
    gap: 5px;
    line-height: 1;
    flex-shrink: 0;
    padding: 0 4px;
    pointer-events: none;
  }
  .msg-row.last .msg-meta,
  .msg-row:hover .msg-meta,
  .msg-meta.always-visible {
    display: flex;
    pointer-events: auto;
  }
  .msg-time {
    font-size: 10.5px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  /* Message-status indicator (icon-only, color carries the meaning).
     Meta lives outside the bubble now, so colors read on the chat bg. */
  .msg-status {
    display: inline-flex;
    align-items: center;
    line-height: 0;
    color: var(--text-muted);
  }
  /* Pending direct send - neutral, soft pulse via the spinner itself. */
  .msg-status.sending {
    opacity: 0.85;
  }
  /* Waiting for offline delivery - slow upward "lift" pulse. */
  .msg-status.queued {
    color: var(--text-muted);
    animation: msg-status-pulse 2.4s ease-in-out infinite;
  }
  /* Direct delivered - single check. */
  .msg-status.delivered {
    color: var(--text-secondary);
  }
  /* Delivered via the offline mailbox - reached the peer's device but not
     necessarily read. Neutral tone so it never reads as the "read" state. */
  .msg-status.offline-delivered {
    color: var(--text-secondary);
  }
  .msg-status.failed {
    color: var(--danger);
  }
  /* Read by the peer - double check in success green. */
  .msg-status.read {
    color: var(--success);
  }

  /* Received-offline marker: a received message that came in via the DHT
     store-and-forward channel while you were away, rather than live. Sits
     under the message body, muted so it never competes with the text. */
  .offline-recv {
    display: inline-flex;
    align-items: center;
    align-self: flex-start;
    gap: 4px;
    margin-top: 2px;
    font-size: 10.5px;
    line-height: 1.3;
    color: var(--text-muted);
    opacity: 0.9;
  }
  .offline-recv :global(svg) {
    flex-shrink: 0;
    opacity: 0.85;
  }
  @keyframes msg-status-pulse {
    0%,
    100% {
      opacity: 0.55;
    }
    50% {
      opacity: 1;
    }
  }

  /* Inline discard - sits beside the bubble (toward the chat center for
     sent messages, the chat edge for received) instead of below it. The
     lifecycle icon next to the time communicates state; this is purely a
     per-message cancel affordance. */
  .bubble-row {
    display: flex;
    align-items: center;
    gap: 12px;
    max-width: 100%;
  }
  .bubble-row.sent {
    flex-direction: row;
    justify-content: flex-end;
  }
  .inline-actions {
    display: inline-flex;
    flex-shrink: 0;
    gap: 4px;
  }
  .inline-action {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    transition: all var(--transition);
    animation: inline-discard-in 220ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .inline-action.discard {
    background: var(--danger-dim);
  }
  .inline-action.discard.queued {
    background: var(--accent-dim);
  }
  .inline-action.retry {
    background: var(--accent-dim);
  }
  .inline-action:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  @keyframes inline-discard-in {
    from {
      opacity: 0;
      transform: scale(0.6);
    }
    to {
      opacity: 1;
      transform: scale(1);
    }
  }

  /* Stacked: bubble, then reaction chips, then the time/status line. A row
     layout put the meta beside the bubble, so revealing it shoved the bubble
     sideways and left the chips free to overlap it. */
  .bubble-anchor {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 3px;
    max-width: 100%;
    min-width: 0;
    position: relative;
  }
  .bubble-anchor.sent {
    align-self: flex-end;
    align-items: flex-end;
  }

  .msg-actions {
    position: absolute;
    top: 0;
    left: auto;
    right: 4px;
    transform: translateY(-50%);
    z-index: 100;
    display: flex;
    gap: 2px;
    background: var(--surface);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    padding: 3px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border-light);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.16);
    opacity: 0;
    animation: fadeInActions 120ms ease forwards;
  }
  @keyframes fadeInActions {
    to {
      opacity: 1;
    }
  }

  .act-btn {
    width: 30px;
    height: 30px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    transition: all var(--transition);
  }
  .act-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .act-btn.active {
    background: var(--accent-dim);
    color: var(--accent-hover);
  }

  .act-more {
    position: relative;
    display: flex;
  }
  .act-menu {
    position: fixed;
    min-width: 168px;
    padding: 5px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: var(--surface);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid var(--border-light);
    border-radius: var(--radius-md);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.24);
    z-index: 200;
    transform-origin: top right;
    animation: actMenuIn 130ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  .act-menu.sent {
    transform-origin: top right;
  }
  .act-menu.up {
    transform-origin: bottom right;
    animation: actMenuUpIn 130ms cubic-bezier(0.22, 1, 0.36, 1);
  }
  @keyframes actMenuIn {
    from {
      opacity: 0;
      transform: scale(0.94) translateY(-4px);
    }
  }
  @keyframes actMenuUpIn {
    from {
      opacity: 0;
      transform: scale(0.94) translateY(4px);
    }
  }
  .menu-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 9px;
    border-radius: var(--radius-sm);
    font-size: 13px;
    color: var(--text-secondary);
    text-align: left;
    white-space: nowrap;
    transition:
      background var(--transition),
      color var(--transition);
  }
  .menu-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .menu-item.danger:hover {
    background: rgba(248, 113, 113, 0.15);
    color: var(--danger);
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
    border-left: 3px solid var(--file-color, var(--text-muted));
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
  .bubble:not(.sent) .file-bubble.embedded {
    border-top-color: var(--border-light);
  }
  .file-icon {
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, var(--file-color, var(--text-muted)) 18%, transparent);
    border-radius: var(--radius-md);
    color: var(--file-color, currentColor);
    flex-shrink: 0;
  }
  .bubble.sent .file-icon {
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
    font-size: 11px;
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
  .bubble.sent .file-dl-btn.ghost {
    background: rgba(255, 255, 255, 0.18);
    color: #fff;
  }
  .file-dl-btn:hover:not(:disabled) {
    background: var(--accent-hover);
    transform: scale(1.05);
  }
  .file-dl-btn.ghost:hover:not(:disabled) {
    background: var(--bg-hover);
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
  .file-progress-ring .ring-fill {
    fill: none;
    stroke: var(--accent);
    stroke-width: 3;
    stroke-linecap: round;
    transition: stroke-dasharray 200ms linear;
  }
  .bubble.sent .file-progress-ring .ring-track {
    stroke: rgba(255, 255, 255, 0.2);
  }
  .file-progress-ring .ring-fill.on-sent,
  .bubble.sent .file-progress-ring .ring-fill {
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
  .bubble.sent .ring-pct {
    color: #fff;
  }

  @media (max-width: 768px) {
    .msg-actions {
      display: none;
    }
    .bubble {
      font-size: 15px;
    }
  }

  /* Touch users: also kill the long-press callout menu. */
  .msg-content:not(.selectable) {
    -webkit-touch-callout: none;
  }

  /* ── Flat layout ─────────────────────────────────────────────────────── */
  .msg-row[data-layout='flat'] {
    align-items: flex-start;
    border-radius: 0;
    padding: 0;
    background: transparent !important;
  }
  .msg-row[data-layout='flat'].sent {
    align-items: flex-start;
  }
  .msg-row[data-layout='flat'] .bubble-anchor {
    max-width: 100%;
    display: block;
    width: 100%;
    align-self: flex-start;
  }
  .msg-row[data-layout='flat'] .bubble,
  .msg-row[data-layout='flat'].received:not(.first) .bubble,
  .msg-row[data-layout='flat'].received:not(.last) .bubble,
  .msg-row[data-layout='flat'].sent:not(.first) .bubble,
  .msg-row[data-layout='flat'].sent:not(.last) .bubble {
    background: transparent;
    border-radius: 2px;
    box-shadow: none;
    animation: none;
    padding: 1px 0;
    max-width: 100%;
    width: 100%;
  }
  .msg-row[data-layout='flat'] .bubble.sent {
    background: transparent;
    color: var(--text-primary);
  }
  .msg-row[data-layout='flat'] .bubble.failed {
    background: transparent;
    color: var(--danger);
    animation: none;
  }
  .msg-row[data-layout='flat'] .bubble.queued {
    background: transparent;
    color: var(--text-muted);
    outline: none;
    animation: none;
  }
  .msg-row[data-layout='flat'] .bubble.sending {
    opacity: 0.6;
  }
  .msg-row[data-layout='flat'] .bubble.sending::after {
    display: none;
  }
  /* flat: time + status are owned by the layout (gutter time on hover for
     follow-up rows, the group's status in the lead header), so the in-bubble
     meta is hidden - it would otherwise duplicate over the avatar gutter. */
  .msg-row[data-layout='flat'] .msg-meta {
    display: none;
  }
  .msg-row[data-layout='flat'] .reply-ref,
  .msg-row[data-layout='flat'] .reply-ref.sent {
    align-self: flex-start;
    border-radius: var(--radius-sm);
  }
  .msg-row[data-layout='flat'] .reactions.sent {
    justify-content: flex-start;
  }
  /* flat collapses .bubble-anchor to a block, so restate the stack spacing. */
  .msg-row[data-layout='flat'] .reactions {
    margin-top: 2px;
  }
</style>
