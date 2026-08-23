<script lang="ts">
  import { Pin, BellOff, MoreHorizontal } from 'lucide-svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { messagesState } from '$lib/state/messages.svelte';
  import { pinnedConvosState } from '$lib/state/pinnedConvos.svelte';
  import { typingState } from '$lib/state/typing.svelte';
  import { draftsState } from '$lib/state/drafts.svelte';
  import { formatTimeShort } from '$lib/utils/dateFormat.svelte';
  import Avatar from '$lib/components/Avatar.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import { t } from '$lib/i18n';
  import type { ContactResponse } from '$lib/types';

  let {
    contact,
    index,
    active,
    statusLabel,
    lastMessageTs,
    renaming,
    renameValue = $bindable(''),
    onOpen,
    onOpenMenu,
    onLongPressStart,
    onLongPressCancel,
    onCommitRename,
    onCancelRename,
  }: {
    contact: ContactResponse;
    index: number;
    active: boolean;
    statusLabel: string;
    lastMessageTs: number;
    renaming: boolean;
    renameValue?: string;
    onOpen: () => void;
    onOpenMenu: (x: number, y: number) => void;
    onLongPressStart: (e: TouchEvent) => void;
    onLongPressCancel: () => void;
    onCommitRename: () => void;
    onCancelRename: () => void;
  } = $props();

  const unread = $derived(messagesState.unreadFor(contact.userId));
  const unreadCapped = $derived(messagesState.unreadCappedFor(contact.userId));
  const status = $derived(contactsState.connectionStatus[contact.userId]);
  const pinned = $derived(pinnedConvosState.has(contact.userId));

  function focusOnMount(node: HTMLInputElement) {
    node.focus();
    node.select();
  }
</script>

<div class="contact-row-wrap">
  <button
    class="contact-row"
    class:active
    class:pinned-convo={pinned}
    class:unread={unread > 0}
    style="animation-delay: {Math.min(index * 30, 180)}ms"
    oncontextmenu={(e) => {
      e.preventDefault();
      onOpenMenu(e.clientX, e.clientY);
    }}
    ontouchstart={onLongPressStart}
    ontouchend={onLongPressCancel}
    ontouchmove={onLongPressCancel}
    onclick={onOpen}
  >
    <div class="contact-avatar">
      <Avatar name={contact.displayName} src={contact.avatarPath} size={42} />
      <StatusDot status={status ?? 'disconnected'} label={statusLabel} />
    </div>
    <div class="contact-meta">
      <div class="contact-top">
        {#if pinned}
          <span class="convo-pin"><Pin size={11} /></span>
        {/if}
        {#if renaming}
          <input
            class="rename-input"
            bind:value={renameValue}
            maxlength="32"
            placeholder={contact.profileName ?? contact.displayName}
            use:focusOnMount
            onclick={(e) => e.stopPropagation()}
            onblur={onCancelRename}
            onkeydown={(e) => {
              e.stopPropagation();
              if (e.key === 'Enter') onCommitRename();
              if (e.key === 'Escape') onCancelRename();
            }}
          />
        {:else}
          <span class="contact-name">{contact.displayName}</span>
        {/if}
        {#if contactsState.isMuted(contact.userId)}
          <span class="convo-muted" title={t('layout.contactMenu.mutedIndicator')}>
            <BellOff size={11} />
          </span>
        {/if}
        {#if lastMessageTs}
          <span class="contact-time">{formatTimeShort(lastMessageTs)}</span>
        {/if}
      </div>
      <div class="contact-bottom">
        {#if typingState.isTyping(contact.userId)}
          <span class="contact-preview typing">{t('layout.typing')}</span>
        {:else if draftsState.get(contact.userId)}
          <span class="contact-preview draft">{t('layout.draftIndicator')}</span>
        {:else}
          <span class="contact-status-text">{statusLabel}</span>
        {/if}
        {#if unread > 0}
          <span class="badge">{unread > 99 || unreadCapped ? '99+' : unread}</span>
        {/if}
      </div>
    </div>
  </button>
  <button
    class="row-menu-btn"
    aria-label={t('layout.contactMenu.more')}
    onclick={(e) => {
      e.stopPropagation();
      onOpenMenu(e.clientX, e.clientY);
    }}
  >
    <MoreHorizontal size={16} />
  </button>
</div>

<style>
  .contact-row-wrap {
    position: relative;
  }
  .contact-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border-radius: var(--radius-md);
    text-align: left;
    transition:
      background var(--transition),
      transform var(--transition),
      box-shadow var(--transition);
    margin-bottom: 1px;
    animation: row-slide-in 0.22s cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  @keyframes row-slide-in {
    from {
      opacity: 0;
      transform: translateX(-8px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
  }
  @media (hover: hover) {
    .contact-row:hover {
      background: var(--bg-hover);
    }
  }
  .contact-row:active {
    background: color-mix(in srgb, var(--accent-solid) 20%, transparent);
  }
  .contact-row.active {
    background: var(--accent-dim);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .contact-row.active .contact-name {
    color: var(--accent-hover);
  }
  .contact-row.pinned-convo {
    background: color-mix(in srgb, var(--accent) 5%, transparent);
  }
  .contact-avatar {
    position: relative;
    flex-shrink: 0;
  }
  .contact-avatar :global(.status-dot) {
    position: absolute;
    bottom: -1px;
    right: -1px;
    border: 2px solid var(--bg-secondary);
  }
  .contact-meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .contact-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .contact-name {
    font-size: 14px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-primary);
    flex: 1;
    min-width: 0;
  }
  .contact-row.unread .contact-name {
    font-weight: 700;
  }
  .rename-input {
    flex: 1;
    min-width: 0;
    background: var(--bg-input);
    border: 1px solid var(--accent-selected);
    border-radius: var(--radius-sm);
    padding: 2px 6px;
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-primary);
  }
  .contact-time {
    font-size: var(--text-2xs);
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .contact-row.unread .contact-time {
    color: var(--accent);
    font-weight: 600;
  }
  .contact-bottom {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .contact-preview,
  .contact-status-text {
    font-size: 12.5px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
    min-width: 0;
  }
  .contact-row.unread .contact-preview {
    color: var(--text-secondary);
  }
  .contact-preview.draft {
    color: var(--accent);
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .contact-preview.typing {
    color: var(--accent);
    font-style: italic;
    animation: typing-pulse 1.4s ease-in-out infinite;
  }
  @keyframes typing-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.55;
    }
  }
  .badge {
    flex-shrink: 0;
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    font-size: var(--text-2xs);
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .convo-pin,
  .convo-muted {
    display: inline-flex;
    align-items: center;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .row-menu-btn {
    position: absolute;
    top: 50%;
    right: 10px;
    transform: translateY(-50%);
    width: 26px;
    height: 26px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    background: var(--bg-tertiary);
    box-shadow: var(--shadow-1);
    opacity: 0;
    pointer-events: none;
    transition:
      opacity var(--transition),
      background var(--transition);
  }
  @media (hover: hover) {
    .row-menu-btn:hover {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
    .contact-row-wrap:hover .row-menu-btn {
      opacity: 1;
      pointer-events: auto;
    }
    .contact-row-wrap:hover .contact-top,
    .contact-row-wrap:hover .contact-bottom {
      padding-right: 28px;
    }
  }
  @media (max-width: 768px) {
    .contact-row {
      padding: 11px 10px;
    }
    /* Row actions are hover-only; on touch use long-press / contextmenu. */
    .row-menu-btn {
      display: none;
    }
  }
</style>
