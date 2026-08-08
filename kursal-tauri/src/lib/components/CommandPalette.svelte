<script lang="ts">
  import { Command } from 'bits-ui';
  import { goto } from '$app/navigation';
  import {
    Search,
    Settings as SettingsIcon,
    UserPlus,
    User,
    Palette,
    ShieldCheck,
    Wifi,
    HardDrive,
    Zap,
    Archive,
    CheckCheck,
    MessageSquare,
    ChevronLeft,
    RefreshCw,
    Maximize2,
    Power,
    type Icon,
  } from 'lucide-svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { messagesState } from '$lib/state/messages.svelte';
  import { uiState } from '$lib/state/ui.svelte';
  import { searchMessagesGlobal } from '$lib/api/messages';
  import { checkForUpdates, closeForceQuit } from '$lib/api/settings';
  import { isMobile, resetWindowSize } from '$lib/api/window';
  import { notifyError } from '$lib/utils/errors';
  import type { MessageResponse } from '$lib/types';
  import { trapFocus } from '$lib/utils/focusTrap';
  import { clockOptions } from '$lib/utils/timeFormat';
  import Avatar from './Avatar.svelte';
  import { t, dateLocale } from '$lib/i18n';

  let { open = $bindable(false), onClose }: { open?: boolean; onClose: () => void } = $props();

  let search = $state('');
  let mode = $state<'default' | 'messages'>('default');
  let messageResults = $state<MessageResponse[]>([]);
  let searchSeq = 0;

  $effect(() => {
    if (!open) {
      search = '';
      mode = 'default';
      messageResults = [];
    }
  });

  // On-demand global message search: only runs once in message-search mode,
  // never on every keystroke of the normal palette.
  $effect(() => {
    if (mode !== 'messages') return;
    const q = search.trim();
    if (!q) {
      messageResults = [];
      return;
    }
    const seq = ++searchSeq;
    const handle = setTimeout(async () => {
      try {
        const res = await searchMessagesGlobal(q, 30);
        if (seq === searchSeq) messageResults = res;
      } catch {
        /* ignore */
      }
    }, 200);
    return () => clearTimeout(handle);
  });

  function enterMessageSearch() {
    mode = 'messages';
    search = '';
    messageResults = [];
  }
  function exitMessageSearch() {
    mode = 'default';
    search = '';
    messageResults = [];
  }
  function jumpToMessage(m: MessageResponse) {
    onClose();
    uiState.pendingMessageJump = { contactId: m.contactId, messageId: m.id };
    goto('/chat/' + m.contactId);
  }
  function contactName(id: string): string {
    return contactsState.getById(id)?.displayName ?? '';
  }
  function contactAvatar(id: string): string | null | undefined {
    return contactsState.getById(id)?.avatarBase64;
  }
  function fmtResultTime(ts: number): string {
    const d = new Date(ts);
    const now = new Date();
    if (d.toDateString() === now.toDateString())
      return d.toLocaleTimeString(dateLocale(), clockOptions());
    return d.toLocaleDateString(dateLocale(), { month: 'short', day: 'numeric' });
  }

  type Cmd = {
    id: string;
    label: string;
    keywords: string[];
    icon?: typeof Icon;
    avatar?: { name: string; src?: string | null };
    run: () => void;
  };

  function nav(path: string) {
    onClose();
    goto(path);
  }

  const conversations = $derived<Cmd[]>(
    [...contactsState.contacts]
      .sort((a, b) => contactsState.activityAt(b) - contactsState.activityAt(a))
      .map((c) => ({
        id: 'contact:' + c.userId,
        label: c.displayName,
        keywords: [c.displayName],
        avatar: { name: c.displayName, src: c.avatarBase64 },
        run: () => nav('/chat/' + c.userId),
      }))
  );

  const settingsCats = [
    { id: 'account', icon: User },
    { id: 'appearance', icon: Palette },
    { id: 'privacy', icon: ShieldCheck },
    { id: 'network', icon: Wifi },
    { id: 'storage', icon: HardDrive },
    { id: 'advanced', icon: Zap },
  ];

  const goTo = $derived<Cmd[]>([
    {
      id: 'settings',
      label: t('commandPalette.openSettings'),
      keywords: [t('commandPalette.openSettings')],
      icon: SettingsIcon,
      run: () => nav('/settings'),
    },
    ...settingsCats.map((s) => {
      const name = t('settings.categories.' + s.id);
      return {
        id: 'settings:' + s.id,
        label: t('commandPalette.settingsCategory', { name }),
        keywords: [name, t('commandPalette.openSettings')],
        icon: s.icon,
        run: () => nav('/settings?cat=' + s.id),
      };
    }),
  ]);

  const addContact = $derived<Cmd[]>([
    {
      id: 'add',
      label: t('commandPalette.openAddContact'),
      keywords: [t('commandPalette.openAddContact')],
      icon: UserPlus,
      run: () => nav('/add-contact'),
    },
    {
      id: 'add:ltc',
      label: t('commandPalette.addContactMethod', {
        name: t('commandPalette.methodLtc'),
      }),
      keywords: [
        t('commandPalette.methodLtc'),
        t('commandPalette.methodOtp'),
        t('commandPalette.methodNearby'),
        t('commandPalette.openAddContact'),
      ],
      icon: Archive,
      run: () => nav('/add-contact'),
    },
  ]);

  const actions = $derived<Cmd[]>([
    {
      id: 'search-messages',
      label: t('commandPalette.searchMessages'),
      keywords: [t('commandPalette.searchMessages'), 'search', 'find'],
      icon: MessageSquare,
      run: enterMessageSearch,
    },
    ...(messagesState.totalUnread() > 0
      ? [
          {
            id: 'mark-all-read',
            label: t('commandPalette.markAllRead'),
            keywords: [t('commandPalette.markAllRead'), 'unread', 'read'],
            icon: CheckCheck,
            run: () => {
              messagesState.markAllRead();
              onClose();
            },
          },
        ]
      : []),
    ...(isMobile
      ? []
      : [
          {
            id: 'check-updates',
            label: t('commandPalette.checkForUpdates'),
            keywords: [t('commandPalette.checkForUpdates'), 'update', 'version', 'upgrade'],
            icon: RefreshCw,
            run: () => {
              onClose();
              checkForUpdates().catch((e) => notifyError(e, 'settings.advanced.errorUpdateCheck'));
            },
          },
          {
            id: 'reset-window-size',
            label: t('commandPalette.resetWindowSize'),
            keywords: [t('commandPalette.resetWindowSize'), 'window', 'size', 'reset'],
            icon: Maximize2,
            run: () => {
              onClose();
              resetWindowSize().catch(notifyError);
            },
          },
          {
            id: 'quit',
            label: t('commandPalette.quit'),
            keywords: [t('commandPalette.quit'), 'quit', 'exit', 'close'],
            icon: Power,
            run: () => {
              onClose();
              closeForceQuit().catch(notifyError);
            },
          },
        ]),
  ]);

  const groups = $derived([
    {
      id: 'conversations',
      heading: t('commandPalette.groupConversations'),
      items: conversations,
    },
    { id: 'goto', heading: t('commandPalette.groupGoTo'), items: goTo },
    {
      id: 'addContact',
      heading: t('commandPalette.groupAddContact'),
      items: addContact,
    },
    { id: 'actions', heading: t('commandPalette.groupActions'), items: actions },
  ]);

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      if (mode === 'messages') exitMessageSearch();
      else onClose();
    }
  }
</script>

{#if open}
  <div class="cp-backdrop" onclick={onClose} role="presentation"></div>
  <div class="cp-wrap" onkeydown={onKeydown} use:trapFocus role="dialog" tabindex="0">
    <Command.Root
      class="cp"
      label={t('commandPalette.ariaLabel')}
      loop
      shouldFilter={mode === 'default'}
    >
      <div class="cp-input-row">
        {#if mode === 'messages'}
          <button
            class="cp-back"
            type="button"
            onclick={exitMessageSearch}
            aria-label={t('common.back')}
          >
            <ChevronLeft size={16} />
          </button>
        {:else}
          <Search size={16} />
        {/if}
        <Command.Input
          bind:value={search}
          class="cp-input"
          placeholder={mode === 'messages'
            ? t('commandPalette.searchMessagesPlaceholder')
            : t('commandPalette.placeholder')}
        />
      </div>
      <Command.List class="cp-list">
        {#if mode === 'messages'}
          {#if !search.trim()}
            <div class="cp-empty">{t('commandPalette.searchMessagesHint')}</div>
          {:else if messageResults.length === 0}
            <div class="cp-empty">{t('commandPalette.empty')}</div>
          {:else}
            {#each messageResults as m (m.id)}
              <Command.Item value={m.id} onSelect={() => jumpToMessage(m)} class="cp-item">
                <Avatar
                  name={contactName(m.contactId)}
                  src={contactAvatar(m.contactId)}
                  size={22}
                />
                <div class="cp-msg-text">
                  <span class="cp-msg-top">
                    <span class="cp-msg-name">{contactName(m.contactId)}</span>
                    <span class="cp-msg-time">{fmtResultTime(m.timestamp)}</span>
                  </span>
                  <span class="cp-label">{m.content}</span>
                </div>
              </Command.Item>
            {/each}
          {/if}
        {:else}
          <Command.Empty class="cp-empty">{t('commandPalette.empty')}</Command.Empty>
          {#each groups as g (g.id)}
            {#if g.items.length}
              <Command.Group class="cp-group">
                <Command.GroupHeading class="cp-heading">{g.heading}</Command.GroupHeading>
                <Command.GroupItems>
                  {#each g.items as it (it.id)}
                    <Command.Item
                      value={it.id}
                      keywords={it.keywords}
                      onSelect={it.run}
                      class="cp-item"
                    >
                      {#if it.avatar}
                        <Avatar name={it.avatar.name} src={it.avatar.src} size={22} />
                      {:else if it.icon}
                        {@const Icon = it.icon}
                        <span class="cp-icon"><Icon size={16} /></span>
                      {/if}
                      <span class="cp-label">{it.label}</span>
                    </Command.Item>
                  {/each}
                </Command.GroupItems>
              </Command.Group>
            {/if}
          {/each}
        {/if}
      </Command.List>
    </Command.Root>
  </div>
{/if}

<style>
  .cp-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    backdrop-filter: blur(2px);
    z-index: 2000;
    animation: cp-fade 120ms ease;
  }
  .cp-wrap {
    position: fixed;
    top: calc(var(--safe-top) + 14vh);
    left: var(--safe-center-x);
    transform: translateX(-50%);
    width: min(560px, calc(var(--safe-w) - 32px));
    z-index: 2001;
    animation: cp-pop 150ms cubic-bezier(0.2, 0.9, 0.3, 1.1);
  }
  @keyframes cp-fade {
    from {
      opacity: 0;
    }
  }
  @keyframes cp-pop {
    from {
      opacity: 0;
      transform: translateX(-50%) translateY(-8px) scale(0.98);
    }
  }

  :global(.cp) {
    display: flex;
    flex-direction: column;
    max-height: min(60vh, 480px, calc(86vh - var(--safe-top) - var(--safe-bottom)));
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
    overflow: hidden;
  }

  .cp-input-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
    color: var(--text-muted);
    flex-shrink: 0;
  }
  :global(.cp-input) {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 15px;
    font-family: inherit;
  }
  :global(.cp-input)::placeholder {
    color: var(--text-muted);
  }
  :global(.cp-input:focus),
  :global(.cp-input:focus-visible) {
    outline: none;
    box-shadow: none;
  }

  :global(.cp-list) {
    overflow-y: auto;
    padding: 6px;
  }
  :global(.cp-empty) {
    padding: 28px 16px;
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
  }
  :global(.cp-heading) {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
    padding: 10px 10px 4px;
  }
  :global(.cp-item) {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-size: 14px;
    cursor: pointer;
    user-select: none;
  }
  :global(.cp-item[data-selected]) {
    background: var(--accent-dim);
    color: var(--accent-hover);
  }
  .cp-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    color: var(--text-secondary);
    flex-shrink: 0;
  }
  :global(.cp-item[data-selected]) .cp-icon {
    color: var(--accent-hover);
  }
  .cp-label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cp-back {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .cp-back:hover {
    color: var(--text-primary);
  }
  .cp-msg-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
    gap: 1px;
  }
  .cp-msg-top {
    display: flex;
    justify-content: space-between;
    gap: 8px;
  }
  .cp-msg-name {
    font-size: 12px;
    font-weight: 700;
    color: var(--text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .cp-msg-time {
    font-size: 11px;
    color: var(--text-muted);
    flex-shrink: 0;
  }
</style>
