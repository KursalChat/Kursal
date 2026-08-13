<script lang="ts">
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
  let selectedId = $state<string | null>(null);
  let listEl = $state<HTMLDivElement | null>(null);

  $effect(() => {
    if (!open) {
      search = '';
      mode = 'default';
      messageResults = [];
      selectedId = null;
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
    selectedId = null;
  }
  function exitMessageSearch() {
    mode = 'default';
    search = '';
    messageResults = [];
    selectedId = null;
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
    return contactsState.getById(id)?.avatarPath;
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
        avatar: { name: c.displayName, src: c.avatarPath },
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

  function matches(it: Cmd, query: string): boolean {
    if (it.label.toLowerCase().includes(query)) return true;
    return it.keywords.some((k) => k.toLowerCase().includes(query));
  }

  const visibleGroups = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return groups
      .map((g) => ({
        ...g,
        items: q ? g.items.filter((it) => matches(it, q)) : g.items,
      }))
      .filter((g) => g.items.length > 0);
  });

  const rows = $derived<{ id: string; run: () => void }[]>(
    mode === 'messages'
      ? messageResults.map((m) => ({ id: m.id, run: () => jumpToMessage(m) }))
      : visibleGroups.flatMap((g) => g.items)
  );

  const selectedKey = $derived.by(() => {
    if (rows.length === 0) return null;
    if (selectedId && rows.some((r) => r.id === selectedId)) return selectedId;
    return rows[0].id;
  });

  $effect(() => {
    if (!selectedKey) return;
    listEl?.querySelector('[data-selected]')?.scrollIntoView({ block: 'nearest' });
  });

  function move(delta: number) {
    if (rows.length === 0) return;
    const cur = rows.findIndex((r) => r.id === selectedKey);
    selectedId = rows[(cur + delta + rows.length) % rows.length].id;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      if (mode === 'messages') exitMessageSearch();
      else onClose();
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      move(1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      move(-1);
    } else if (e.key === 'Home') {
      e.preventDefault();
      if (rows.length) selectedId = rows[0].id;
    } else if (e.key === 'End') {
      e.preventDefault();
      if (rows.length) selectedId = rows[rows.length - 1].id;
    } else if (e.key === 'Enter') {
      e.preventDefault();
      rows.find((r) => r.id === selectedKey)?.run();
    }
  }
</script>

{#if open}
  <div class="cp-backdrop" onclick={onClose} role="presentation"></div>
  <div class="cp-wrap" onkeydown={onKeydown} use:trapFocus role="dialog" tabindex="0">
    <div class="cp">
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
        <input
          class="cp-input"
          type="text"
          autocomplete="off"
          autocorrect="off"
          spellcheck="false"
          role="combobox"
          aria-autocomplete="list"
          aria-expanded="true"
          aria-controls="cp-list"
          aria-activedescendant={selectedKey ? 'cp-opt-' + selectedKey : undefined}
          aria-label={t('commandPalette.ariaLabel')}
          bind:value={search}
          oninput={() => (selectedId = null)}
          placeholder={mode === 'messages'
            ? t('commandPalette.searchMessagesPlaceholder')
            : t('commandPalette.placeholder')}
        />
      </div>
      <div
        class="cp-list"
        id="cp-list"
        role="listbox"
        aria-label={t('commandPalette.ariaLabel')}
        bind:this={listEl}
      >
        {#if mode === 'messages'}
          {#if !search.trim()}
            <div class="cp-empty">{t('commandPalette.searchMessagesHint')}</div>
          {:else if messageResults.length === 0}
            <div class="cp-empty">{t('commandPalette.empty')}</div>
          {:else}
            {#each messageResults as m (m.id)}
              <button
                class="cp-item"
                type="button"
                role="option"
                tabindex="-1"
                id={'cp-opt-' + m.id}
                aria-selected={selectedKey === m.id}
                data-selected={selectedKey === m.id ? '' : undefined}
                onclick={() => jumpToMessage(m)}
                onmousedown={(e) => e.preventDefault()}
                onmousemove={() => (selectedId = m.id)}
              >
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
              </button>
            {/each}
          {/if}
        {:else if rows.length === 0}
          <div class="cp-empty">{t('commandPalette.empty')}</div>
        {:else}
          {#each visibleGroups as g (g.id)}
            <div class="cp-group">
              <div class="cp-heading">{g.heading}</div>
              {#each g.items as it (it.id)}
                <button
                  class="cp-item"
                  type="button"
                  role="option"
                  tabindex="-1"
                  id={'cp-opt-' + it.id}
                  aria-selected={selectedKey === it.id}
                  data-selected={selectedKey === it.id ? '' : undefined}
                  onclick={it.run}
                  onmousedown={(e) => e.preventDefault()}
                  onmousemove={() => (selectedId = it.id)}
                >
                  {#if it.avatar}
                    <Avatar name={it.avatar.name} src={it.avatar.src} size={22} />
                  {:else if it.icon}
                    {@const Icon = it.icon}
                    <span class="cp-icon"><Icon size={16} /></span>
                  {/if}
                  <span class="cp-label">{it.label}</span>
                </button>
              {/each}
            </div>
          {/each}
        {/if}
      </div>
    </div>
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

  .cp {
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
  .cp-input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 15px;
    font-family: inherit;
  }
  .cp-input::placeholder {
    color: var(--text-muted);
  }
  .cp-input:focus,
  .cp-input:focus-visible {
    outline: none;
    box-shadow: none;
  }

  .cp-list {
    overflow-y: auto;
    padding: 6px;
  }
  .cp-empty {
    padding: 28px 16px;
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
  }
  .cp-heading {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
    padding: 10px 10px 4px;
  }
  .cp-item {
    display: flex;
    width: 100%;
    align-items: center;
    text-align: left;
    gap: 10px;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-size: 14px;
    cursor: pointer;
    user-select: none;
  }
  .cp-item[data-selected] {
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
  .cp-item[data-selected] .cp-icon {
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
