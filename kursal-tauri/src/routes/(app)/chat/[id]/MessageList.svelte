<script lang="ts">
  import type { Snippet } from 'svelte';
  import { t } from '$lib/i18n';
  import { CloudUpload, CircleAlert, Check, CheckCheck } from 'lucide-svelte';
  import Avatar from '$lib/components/Avatar.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { appearanceState } from '$lib/state/appearance.svelte';
  import { profileState } from '$lib/state/profile.svelte';
  import { typingState } from '$lib/state/typing.svelte';
  import type { ContactResponse, MessageResponse } from '$lib/types';
  import type { MessageGroup } from './chat-grouping';
  import { formatTime, isSameDay, formatDaySeparator, flatStatusLabel } from './chat-utils';
  import EmptyChat from './EmptyChat.svelte';
  import ChatSeparator from './ChatSeparator.svelte';
  import CallLine from './CallLine.svelte';
  import TypingIndicator from './TypingIndicator.svelte';
  import ImageStackBubble from './ImageStackBubble.svelte';

  interface ImageRun {
    msgs: MessageResponse[];
    startIdx: number;
  }

  interface Props {
    /** The scroll container. Bound so the parent keeps owning scroll behaviour. */
    listEl: HTMLElement | null;
    hoveredMessageId: string | null;
    contact: ContactResponse;
    messageGroups: MessageGroup[];
    isEmpty: boolean;
    gapNotices: { counter: number }[];
    firstUnreadId: string | null;
    loadingOlder: boolean;
    loadingNewer: boolean;
    sending: boolean;
    onScroll: () => void;
    imageRuns: (msgs: MessageResponse[]) => ImageRun[];
    onOpenStackImage: (m: MessageResponse, msgs: MessageResponse[]) => void;
    onDownloadStack: (msgs: MessageResponse[]) => void;
    onStackContextMenu: (e: MouseEvent, m: MessageResponse) => void;
    onSayHi: () => void;
    onVerify: () => void;
    msgBubble: Snippet<[MessageResponse, number, number, boolean]>;
  }

  let {
    listEl = $bindable(),
    hoveredMessageId = $bindable(),
    contact,
    messageGroups,
    isEmpty,
    gapNotices,
    firstUnreadId,
    loadingOlder,
    loadingNewer,
    sending,
    onScroll,
    imageRuns,
    onOpenStackImage,
    onDownloadStack,
    onStackContextMenu,
    onSayHi,
    onVerify,
    msgBubble,
  }: Props = $props();

  const groupFirstId = (g: MessageGroup): string | undefined => g.messages[0]?.id;
</script>

<div class="messages" bind:this={listEl} onscroll={onScroll}>
  {#snippet flatStatus(status: string)}
    {#if status === 'sending'}
      <Spinner size={11} color="currentColor" />
    {:else if status === 'queued' || status === 'queued_in_dht'}
      <CloudUpload size={12} />
    {:else if status === 'delivered'}
      <Check size={12} />
    {:else if status === 'offline_delivered' || status === 'read'}
      <CheckCheck size={12} />
    {:else if status === 'failed'}
      <CircleAlert size={12} />
    {/if}
  {/snippet}

  {#if loadingOlder}
    <div class="loading-older"><Spinner size={16} /></div>
  {/if}
  {#if isEmpty}
    <EmptyChat
      name={contact.displayName}
      avatar={contact.avatarBase64}
      verified={contact.verified}
      {sending}
      {onSayHi}
      {onVerify}
    />
  {:else}
    <div class="msg-spacer"></div>

    {#each messageGroups as group, gi (gi)}
      {#if group.tier > 0 && group.tier !== (gi > 0 ? messageGroups[gi - 1].tier : 0)}
        <ChatSeparator
          variant={group.tier === 1 ? 'offline-stored' : 'offline-waiting'}
          label={group.tier === 1
            ? t('chat.offline.sectionStored')
            : t('chat.offline.sectionWaiting')}
        />
      {/if}
      {#if group.tier === 0 && (gi === 0 || !isSameDay(group.timestamp, messageGroups[gi - 1].timestamp))}
        <ChatSeparator variant="day" label={formatDaySeparator(group.timestamp)} />
      {/if}
      {#if firstUnreadId && groupFirstId(group) === firstUnreadId}
        <ChatSeparator
          variant="unread"
          label={t('chat.conversation.newMessages')}
          ariaLabel={t('chat.conversation.newMessages')}
        />
      {/if}
      {#if group.kind === 'call'}
        {@const rec = group.messages[0]?.callDetails}
        {#if rec}
          <CallLine {rec} direction={group.direction} timestamp={group.timestamp} />
        {/if}
      {:else}
        <div
          class="msg-group"
          class:sent={group.direction === 'sent' && appearanceState.layout === 'bubble'}
        >
          {#if group.tier === 0 && gi > 0 && isSameDay(group.timestamp, messageGroups[gi - 1].timestamp) && group.timestamp - messageGroups[gi - 1].timestamp > 3600000}
            <ChatSeparator variant="time" label={formatTime(group.timestamp)} />
          {/if}

          {#if appearanceState.layout === 'flat'}
            {#each imageRuns(group.messages) as run (run.msgs[0].id)}
              {@const msg = run.msgs[0]}
              {@const mi = run.startIdx}
              <div
                class="flat-row"
                class:group-start={mi === 0}
                class:hovered={hoveredMessageId === msg.id}
                data-dir={group.direction}
                role="presentation"
                onmouseenter={() => (hoveredMessageId = msg.id)}
                onmouseleave={() => (hoveredMessageId = null)}
              >
                <div class="flat-aside">
                  {#if mi === 0}
                    <Avatar
                      name={group.direction === 'received'
                        ? contact.displayName
                        : profileState.displayName}
                      src={group.direction === 'received'
                        ? contact.avatarBase64
                        : profileState.avatarBase64}
                      size={32}
                    />
                  {:else}
                    <span class="flat-thread" aria-hidden="true"></span>
                    <time class="flat-gutter-time">{formatTime(msg.timestamp)}</time>
                  {/if}
                </div>
                <div class="flat-body">
                  {#if mi === 0}
                    <div class="flat-header-line">
                      <span class="flat-sender-name" class:sent={group.direction === 'sent'}>
                        {group.direction === 'received'
                          ? contact.displayName
                          : t('chat.conversation.you')}
                      </span>
                      <span class="flat-prompt" aria-hidden="true">~</span>
                      <time class="flat-time">{formatTime(msg.timestamp)}</time>
                    </div>
                  {/if}
                  {#if run.msgs.length > 1}
                    <ImageStackBubble
                      msgs={run.msgs}
                      direction={group.direction}
                      onOpen={onOpenStackImage}
                      onDownloadAll={onDownloadStack}
                      onContextMenu={onStackContextMenu}
                    />
                  {:else}
                    {@render msgBubble(msg, mi, group.messages.length, true)}
                  {/if}
                </div>
              </div>
            {/each}
            {#if group.direction === 'sent' && gi === messageGroups.length - 1}
              {@const lastStatus = group.messages[group.messages.length - 1]?.status ?? 'delivered'}
              <div class="flat-group-status" data-status={lastStatus}>
                {@render flatStatus(lastStatus)}
                <span>{flatStatusLabel(lastStatus)}</span>
              </div>
            {/if}
          {:else}
            <div class="group-body" class:sent={group.direction === 'sent'}>
              {#if group.direction === 'received'}
                <div class="group-avatar">
                  <Avatar name={contact.displayName} src={contact.avatarBase64} size={28} />
                </div>
              {/if}
              <div class="group-messages" class:sent={group.direction === 'sent'}>
                {#each imageRuns(group.messages) as run (run.msgs[0].id)}
                  {#if run.msgs.length > 1}
                    <ImageStackBubble
                      msgs={run.msgs}
                      direction={group.direction}
                      onOpen={onOpenStackImage}
                      onDownloadAll={onDownloadStack}
                      onContextMenu={onStackContextMenu}
                    />
                  {:else}
                    {@render msgBubble(run.msgs[0], run.startIdx, group.messages.length, false)}
                  {/if}
                {/each}
              </div>
            </div>
          {/if}
        </div>
      {/if}
    {/each}
  {/if}

  {#each gapNotices as gap (gap.counter)}
    <div class="gap-notice" role="note">
      <span>{t('chat.conversation.offlineGapSkipped')}</span>
    </div>
  {/each}

  {#if loadingNewer}
    <div class="loading-older"><Spinner size={16} /></div>
  {/if}

  {#if typingState.isTyping(contact.userId)}
    <TypingIndicator name={contact.displayName} avatar={contact.avatarBase64} />
  {/if}

  <div class="composer-clearance" aria-hidden="true"></div>
</div>

<style>
  .messages {
    flex: 1;
    min-height: 0;
    width: 100%;
    max-width: var(--chat-max);
    margin-inline: auto;
    overflow-y: auto;
    overflow-x: hidden;
    --msg-pad-x: 16px;
    padding: 16px var(--msg-pad-x) 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
    overscroll-behavior: contain;
    -webkit-mask-image: linear-gradient(to bottom, transparent 0, black 14px);
    mask-image: linear-gradient(to bottom, transparent 0, black 14px);
    scrollbar-width: thin;
  }

  .msg-group {
    margin-top: 6px;
  }
  .msg-group + .msg-group {
    margin-top: 10px;
  }

  .loading-older {
    display: flex;
    justify-content: center;
    padding: 10px 0 6px;
  }

  .msg-spacer {
    margin-top: auto;
  }
  /* Real element (not padding) reserving room under the last message for the
     floating composer + queue bar. WebKit drops bottom padding from a scroll
     container's scrollable range, which would hide the last messages behind
     the composer; a flex child is always scrollable-to. */
  .composer-clearance {
    flex: 0 0 auto;
    height: calc(var(--composer-h, 76px) + 24px);
  }

  .gap-notice {
    display: flex;
    justify-content: center;
    align-self: center;
    margin: 8px auto;
    padding: 4px 12px;
    font-size: 11.5px;
    font-style: italic;
    color: var(--text-muted);
  }

  .group-body {
    display: flex;
    gap: 8px;
    max-width: 100%;
    align-items: flex-end;
  }
  .group-body.sent {
    justify-content: flex-end;
  }

  .group-avatar {
    flex-shrink: 0;
    align-self: flex-end;
    margin-bottom: 2px;
    position: sticky;
    bottom: 4px;
    z-index: 1;
  }

  .group-messages {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-width: min(72%, 620px);
    min-width: 0;
  }
  .group-messages.sent {
    align-items: flex-end;
  }

  /* ── Flat layout (terminal-hybrid) ──────────────────────────────
     Avatar leads each sender group; follow-up lines are stitched by an
     accent "thread" rule down the gutter, colored by who is speaking.
     Name + time render as a mono prompt so the log reads like a terminal
     while the message body stays in the friendly body face. */
  .flat-row {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 10px;
    /* bleed into .messages padding so hover bg spans full width and adjacent
       rows highlight as one continuous block */
    margin: 0 calc(-1 * var(--msg-pad-x, 16px));
    padding: 1px 16px;
    border-radius: 0;
  }
  .flat-row.group-start {
    margin-top: 12px;
  }
  .flat-row:hover,
  .flat-row.hovered {
    background: var(--bg-hover);
  }
  .flat-aside {
    position: relative;
    width: 34px;
    flex-shrink: 0;
    align-self: stretch;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 1px;
  }
  /* thread rule: a continuous vertical line descending from the avatar,
     tinting the whole group by direction (accent = you, hairline = them) */
  .flat-thread {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 50%;
    width: 2px;
    transform: translateX(-50%);
    border-radius: 2px;
    background: var(--border);
    transition: opacity 0.14s ease;
  }
  .flat-row[data-dir='sent'] .flat-thread {
    background: color-mix(in srgb, var(--accent) 60%, transparent);
  }
  /* follow-up rows swap their thread segment for a mono timestamp on hover */
  .flat-gutter-time {
    position: absolute;
    top: 0;
    right: 2px;
    height: 20px;
    display: flex;
    align-items: center;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    line-height: 1;
    opacity: 0;
    transition: opacity 0.14s ease;
    white-space: nowrap;
    pointer-events: none;
  }
  .flat-row:hover .flat-thread,
  .flat-row.hovered .flat-thread {
    opacity: 0;
  }
  .flat-row:hover .flat-gutter-time,
  .flat-row.hovered .flat-gutter-time {
    opacity: 1;
  }
  .flat-body {
    flex: 1;
    min-width: 0;
    padding-right: 8px;
  }
  .flat-header-line {
    display: flex;
    align-items: baseline;
    gap: 6px;
    margin-bottom: 2px;
  }
  .flat-sender-name {
    font-family: var(--font-mono);
    font-size: 12.5px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--text-primary);
    line-height: 1.2;
  }
  .flat-sender-name.sent {
    color: var(--accent);
  }
  .flat-prompt {
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.2;
  }
  .flat-time {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    line-height: 1;
  }
  /* one status line per group, sitting just below the group's last message and
     aligned under the message text (Slack/iMessage-style) */
  .flat-group-status {
    display: flex;
    align-items: center;
    gap: 5px;
    padding-left: 44px;
    margin-top: 3px;
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }
  .flat-group-status :global(svg) {
    flex-shrink: 0;
  }
  .flat-group-status[data-status='offline_delivered'] {
    color: var(--text-secondary);
  }
  .flat-group-status[data-status='read'] {
    color: var(--success);
  }
  .flat-group-status[data-status='failed'] {
    color: var(--danger);
  }

  @media (max-width: 768px) {
    .messages {
      padding: 12px 10px 48px;
    }
    .group-messages {
      max-width: 82%;
    }
  }

  @media (max-width: 480px) {
    .group-messages {
      max-width: 86%;
    }
  }
</style>
