<script lang="ts">
  import { goto } from '$app/navigation';
  import { log } from '$lib/utils/log';
  import {
    UserPlus,
    Settings as SettingsIcon,
    X,
    MessageSquare,
    Search,
    Pin,
    Check,
    Trash2,
    MoreHorizontal,
    BellOff,
    Bell,
    Archive,
    ChevronDown,
    Pencil,
    Ban,
    Mic,
    MicOff,
    Headphones,
    HeadphoneOff,
    Mail,
  } from 'lucide-svelte';
  import { connectionLabel } from '$lib/utils/connectionLabel';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { messagesState } from '$lib/state/messages.svelte';
  import { profileState } from '$lib/state/profile.svelte';
  import { draftsState } from '$lib/state/drafts.svelte';
  import { pinnedConvosState } from '$lib/state/pinnedConvos.svelte';
  import { archivedConvosState } from '$lib/state/archivedConvos.svelte';
  import { typingState } from '$lib/state/typing.svelte';
  import { uiState } from '$lib/state/ui.svelte';
  import { removeContact, setContactBlocked } from '$lib/api/contacts';
  import { notifications } from '$lib/state/notifications.svelte';
  import { confirmDialog } from '$lib/state/confirm.svelte';
  import { OS, isMobile } from '$lib/api/window';
  import { clockOptions } from '$lib/utils/timeFormat';
  import Avatar from '$lib/components/Avatar.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import CallDock from '$lib/components/CallDock.svelte';
  import { callState } from '$lib/state/call.svelte';
  import OfflineSyncIndicator from '$lib/components/OfflineSyncIndicator.svelte';
  import type { ContactResponse, ConnectionChangedPayload } from '$lib/types';
  import { readInsets } from '$lib/utils/android-insets';
  import * as haptics from '$lib/utils/haptics';
  import { t, dateLocale } from '$lib/i18n';

  interface Props {
    contacts: ContactResponse[];
    currentChatId: string | undefined;
    onOpenCommand: () => void;
  }

  let { contacts, currentChatId, onOpenCommand }: Props = $props();

  const totalUnread = $derived(messagesState.totalUnread());
  const searchHint = OS === 'macos' ? '⌘K' : 'Ctrl K';

  const activeContacts = $derived(contacts.filter((c) => !archivedConvosState.has(c.userId)));
  const archivedList = $derived(contacts.filter((c) => archivedConvosState.has(c.userId)));
  const archivedUnread = $derived(
    archivedList.reduce((n, c) => n + messagesState.unreadFor(c.userId), 0)
  );
  let showArchived = $state(false);

  // Audio device pickers live in the bottom user panel (mic = input, speaker = output).
  let audioMenu = $state<null | 'input' | 'output'>(null);
  const audioDevices = $derived(callState.devices);

  function toggleAudioMenu(kind: 'input' | 'output') {
    if (audioMenu === kind) {
      audioMenu = null;
      return;
    }
    audioMenu = kind;
    callState.refreshDevices();
  }

  function pickDevice(kind: 'input' | 'output', name: string | null) {
    callState.selectDevice(kind, name);
    audioMenu = null;
  }

  // Close the device dropdown on any pointer press outside the controls.
  // pointerdown fires before click, and the toggle lives inside .self-controls,
  // so opening/closing via the chevron never self-cancels.
  function onWindowPointerDown(e: PointerEvent) {
    if (!audioMenu) return;
    if ((e.target as HTMLElement)?.closest('.self-controls')) return;
    audioMenu = null;
  }

  // Drawer swipe: grab from the screen edge to open, drag anywhere on the panel
  // to close. Only in drawer mode, which the CSS switches on at the same width.
  const DRAWER_MAX_WIDTH = 768;
  const EDGE_ZONE = 20;
  const AXIS_LOCK = 10;
  const COMMIT = 72;

  let asideEl = $state<HTMLElement | null>(null);
  let swipe: { x: number; y: number; opening: boolean; locked: boolean } | null = null;

  function drawerWidth(): number {
    return asideEl?.offsetWidth || 300;
  }

  function onDrawerTouchStart(e: TouchEvent) {
    swipe = null;
    if (e.touches.length !== 1 || window.innerWidth > DRAWER_MAX_WIDTH) return;
    const touch = e.touches[0];
    const open = uiState.mobileSidebarOpen;
    if (!open && touch.clientX > readInsets().left + EDGE_ZONE) return;
    if (open && !asideEl?.contains(e.target as Node)) return;
    swipe = { x: touch.clientX, y: touch.clientY, opening: !open, locked: false };
  }

  function onDrawerTouchMove(e: TouchEvent) {
    if (!swipe) return;
    const touch = e.touches[0];
    const dx = touch.clientX - swipe.x;
    const dy = touch.clientY - swipe.y;
    if (!swipe.locked) {
      if (Math.abs(dx) < AXIS_LOCK && Math.abs(dy) < AXIS_LOCK) return;
      // A vertical intent belongs to the contact list, so drop the gesture.
      if (Math.abs(dy) >= Math.abs(dx)) {
        swipe = null;
        return;
      }
      swipe.locked = true;
    }
    const width = drawerWidth();
    const offset = (swipe.opening ? 0 : width) + dx;
    uiState.sidebarDrag = Math.max(0, Math.min(1, offset / width));
  }

  function onDrawerTouchEnd() {
    if (!swipe) return;
    const progress = uiState.sidebarDrag;
    if (progress !== null) {
      const threshold = Math.min(COMMIT, drawerWidth() * 0.35) / drawerWidth();
      const open = swipe.opening ? progress > threshold : progress > 1 - threshold;
      if (open !== uiState.mobileSidebarOpen) {
        uiState.mobileSidebarOpen = open;
        void haptics.impact('light');
      }
    }
    swipe = null;
    uiState.sidebarDrag = null;
  }

  function handleAddContact() {
    uiState.mobileSidebarOpen = false;
    goto('/add-contact');
  }

  function handleSettings() {
    uiState.mobileSidebarOpen = false;
    goto('/settings');
  }

  function getLastMessageTs(contactId: string): number {
    return Math.max(
      messagesState.lastTimestampFor(contactId),
      contactsState.lastMessageAt(contactId)
    );
  }

  function formatTimeShort(ts: number): string {
    if (!ts) return '';
    const d = new Date(ts);
    const now = new Date();
    const startOfToday = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
    if (ts >= startOfToday) {
      return d.toLocaleTimeString(dateLocale(), clockOptions());
    }
    if (ts >= startOfToday - 6 * 24 * 3600 * 1000) {
      return d.toLocaleDateString(dateLocale(), { weekday: 'short' });
    }
    return d.toLocaleDateString(dateLocale(), { month: 'short', day: 'numeric' });
  }

  function getStatusLabel(
    status: ConnectionChangedPayload['status'] | undefined,
    contactId?: string
  ): string {
    return connectionLabel(status, contactId ? contactsState.lastSeenAt(contactId) : null);
  }

  let contactMenu = $state<{ userId: string; x: number; y: number } | null>(null);
  let contactLongPress: ReturnType<typeof setTimeout> | null = null;
  function openContactMenu(userId: string, x: number, y: number) {
    const safe = readInsets();
    contactMenu = {
      userId,
      x: Math.max(safe.left + 8, Math.min(x, window.innerWidth - safe.right - 200)),
      y: Math.max(safe.top + 8, Math.min(y, window.innerHeight - safe.bottom - 260)),
    };
  }
  function startContactLongPress(e: TouchEvent, userId: string) {
    const tp = e.touches[0];
    contactLongPress = setTimeout(() => openContactMenu(userId, tp.clientX, tp.clientY), 500);
  }
  function cancelContactLongPress() {
    if (contactLongPress) {
      clearTimeout(contactLongPress);
      contactLongPress = null;
    }
  }
  let renamingId = $state<string | null>(null);
  let renameValue = $state('');

  function startRename(userId: string) {
    renameValue = contactsState.aliasFor(userId) ?? '';
    renamingId = userId;
    contactMenu = null;
  }

  async function commitRename(userId: string) {
    try {
      await contactsState.setAlias(userId, renameValue);
    } catch (e) {
      notifications.push(t('profile.errorNickname'), 'error');
      log.error(e);
    }
    renamingId = null;
  }

  function focusOnMount(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  async function handleToggleBlock(userId: string) {
    contactMenu = null;
    const c = contactsState.getById(userId);
    if (!c) return;
    const willBlock = !c.blocked;
    const ok = await confirmDialog({
      title: t(willBlock ? 'profile.blockConfirmTitle' : 'profile.unblockConfirmTitle', {
        name: c.displayName,
      }),
      message: t(willBlock ? 'profile.blockConfirmMessage' : 'profile.unblockConfirmMessage'),
      tone: 'warning',
      confirmLabel: t(willBlock ? 'profile.blockConfirmLabel' : 'profile.unblockConfirmLabel'),
    });
    if (!ok) return;
    try {
      await setContactBlocked(userId, willBlock);
      contactsState.upsert({ ...c, blocked: willBlock });
      notifications.push(
        t(willBlock ? 'profile.successBlocked' : 'profile.successUnblocked', {
          name: c.displayName,
        }),
        'success'
      );
    } catch (e) {
      notifications.push(t(willBlock ? 'profile.errorBlock' : 'profile.errorUnblock'), 'error');
      log.error(e);
    }
  }

  async function handleDeleteContact(userId: string) {
    contactMenu = null;
    const c = contactsState.getById(userId);
    const ok = await confirmDialog({
      title: t('layout.contactMenu.deleteTitle'),
      message: t('layout.contactMenu.deleteMessage', {
        name: c?.displayName ?? '',
      }),
      confirmLabel: t('layout.contactMenu.deleteConfirm'),
      tone: 'danger',
    });
    if (!ok) return;
    try {
      await removeContact(userId);
      contactsState.remove(userId);
      if (currentChatId === userId) goto('/chat', { replaceState: true });
    } catch (e) {
      log.error('delete contact failed', e);
    }
  }
</script>

<svelte:window
  onpointerdown={onWindowPointerDown}
  ontouchstart={onDrawerTouchStart}
  ontouchmove={onDrawerTouchMove}
  ontouchend={onDrawerTouchEnd}
  ontouchcancel={onDrawerTouchEnd}
/>

<aside
  class="sidebar"
  class:open={uiState.mobileSidebarOpen}
  class:dragging={uiState.sidebarDrag !== null}
  style={uiState.sidebarDrag !== null
    ? `transform: translateX(${(uiState.sidebarDrag - 1) * 100}%)`
    : ''}
  bind:this={asideEl}
>
  <div class="sidebar-header" data-tauri-drag-region>
    <button
      class="brand"
      onclick={() => {
        uiState.mobileSidebarOpen = false;
        goto('/');
      }}
    >
      <h1>{t('layout.appName')}<span class="prompt">:~$</span></h1>
    </button>
    <div class="header-actions">
      <button
        class="icon-btn"
        onclick={handleAddContact}
        title={t('layout.addContact')}
        aria-label={t('layout.addContact')}
        data-tour="add-contact-btn"
      >
        <UserPlus size={17} />
      </button>
      <button
        class="icon-btn"
        onclick={handleSettings}
        title={t('layout.settings')}
        aria-label={t('layout.settings')}
      >
        <SettingsIcon size={17} />
      </button>
      <button
        class="icon-btn mobile-close"
        onclick={() => (uiState.mobileSidebarOpen = false)}
        aria-label={t('layout.closeMenu')}
      >
        <X size={18} />
      </button>
    </div>
  </div>

  <button class="search-trigger" onclick={onOpenCommand}>
    <Search size={14} />
    <span class="search-trigger-text">{t('layout.searchPlaceholder')}</span>
    {#if !isMobile}
      <kbd class="search-kbd">{searchHint}</kbd>
    {/if}
  </button>

  <div class="contacts-list">
    {#if contactsState.loading}
      <div class="skeleton-list" aria-label={t('layout.loadingContacts')}>
        {#each Array(5) as _, i}
          <div class="skeleton-row" style="animation-delay: {i * 80}ms">
            <div class="sk-avatar"></div>
            <div class="sk-meta">
              <div class="sk-line sk-name"></div>
              <div class="sk-line sk-preview"></div>
            </div>
          </div>
        {/each}
      </div>
    {:else if contactsState.contacts.length === 0}
      <div class="empty-state column">
        <div class="empty-glyph"><MessageSquare size={24} /></div>
        <span>{t('layout.noContacts')}</span>
        <button class="empty-cta" onclick={handleAddContact} data-tour="add-contact-empty">
          <UserPlus size={14} />
          {t('layout.addFirstContact')}
        </button>
      </div>
    {:else}
      {#snippet contactRow(contact: ContactResponse, ci: number)}
        {@const unread = messagesState.unreadFor(contact.userId)}
        {@const unreadCapped = messagesState.unreadCappedFor(contact.userId)}
        {@const status = contactsState.connectionStatus[contact.userId]}
        {@const previewTs = getLastMessageTs(contact.userId)}
        <div class="contact-row-wrap">
          <button
            class="contact-row"
            class:active={currentChatId === contact.userId}
            class:pinned-convo={pinnedConvosState.has(contact.userId)}
            style="animation-delay: {Math.min(ci * 30, 180)}ms"
            class:unread={unread > 0}
            oncontextmenu={(e) => {
              e.preventDefault();
              openContactMenu(contact.userId, e.clientX, e.clientY);
            }}
            ontouchstart={(e) => startContactLongPress(e, contact.userId)}
            ontouchend={cancelContactLongPress}
            ontouchmove={cancelContactLongPress}
            onclick={() => {
              if (contactMenu) return;
              goto('/chat/' + contact.userId);
              uiState.mobileSidebarOpen = false;
            }}
          >
            <div class="contact-avatar">
              <Avatar name={contact.displayName} src={contact.avatarPath} size={42} />
              <StatusDot
                status={status ?? 'disconnected'}
                label={getStatusLabel(status, contact.userId)}
              />
            </div>
            <div class="contact-meta">
              <div class="contact-top">
                {#if pinnedConvosState.has(contact.userId)}
                  <span class="convo-pin"><Pin size={11} /></span>
                {/if}
                {#if renamingId === contact.userId}
                  <input
                    class="rename-input"
                    bind:value={renameValue}
                    maxlength="32"
                    placeholder={contact.profileName ?? contact.displayName}
                    use:focusOnMount
                    onclick={(e) => e.stopPropagation()}
                    onblur={() => (renamingId = null)}
                    onkeydown={(e) => {
                      e.stopPropagation();
                      if (e.key === 'Enter') void commitRename(contact.userId);
                      if (e.key === 'Escape') renamingId = null;
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
                {#if previewTs}
                  <span class="contact-time">{formatTimeShort(previewTs)}</span>
                {/if}
              </div>
              <div class="contact-bottom">
                {#if typingState.isTyping(contact.userId)}
                  <span class="contact-preview typing">{t('layout.typing')}</span>
                {:else if draftsState.get(contact.userId)}
                  <span class="contact-preview draft">{t('layout.draftIndicator')}</span>
                {:else}
                  <span class="contact-status-text">{getStatusLabel(status, contact.userId)}</span>
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
              openContactMenu(contact.userId, e.clientX, e.clientY);
            }}
          >
            <MoreHorizontal size={16} />
          </button>
        </div>
      {/snippet}

      {#each activeContacts as contact, ci (contact.userId)}
        {@render contactRow(contact, ci)}
      {/each}

      {#if archivedList.length > 0}
        <button class="archived-header" onclick={() => (showArchived = !showArchived)}>
          <Archive size={13} />
          <span>{t('layout.archived')} ({archivedList.length})</span>
          {#if archivedUnread > 0}
            <span class="badge">{archivedUnread > 99 ? '99+' : archivedUnread}</span>
          {/if}
          <ChevronDown
            size={14}
            style="margin-left: auto; transform: rotate({showArchived
              ? 0
              : -90}deg); transition: transform 150ms ease"
          />
        </button>
        {#if showArchived}
          {#each archivedList as contact, ci (contact.userId)}
            {@render contactRow(contact, ci)}
          {/each}
        {/if}
      {/if}
    {/if}
  </div>

  <div class="sync-slot">
    <OfflineSyncIndicator />
  </div>

  <div class="dock-slot">
    <CallDock />
  </div>

  <div class="user-panel">
    <button class="user-identity" onclick={handleSettings} aria-label={t('layout.openSettings')}>
      <div class="user-avatar">
        <Avatar name={profileState.displayName} src={profileState.avatarPath} size={36} />
        {#if totalUnread > 0}
          <span class="badge total">{totalUnread > 99 ? '99+' : totalUnread}</span>
        {/if}
      </div>
      <div class="user-info">
        <span class="user-name">{profileState.displayName}</span>
        <span class="user-id"
          >{profileState.peerId ? profileState.peerId.slice(0, 10) + '...' : '...'}</span
        >
      </div>
    </button>

    <div class="self-controls">
      <div class="ctl-grp" class:on={callState.muted}>
        <button
          class="ctl-tgl"
          aria-pressed={callState.muted}
          aria-label={callState.muted ? t('chat.call.unmute') : t('chat.call.mute')}
          title={callState.muted ? t('chat.call.unmute') : t('chat.call.mute')}
          onclick={() => callState.toggleMute()}
        >
          {#if callState.muted}<MicOff size={16} />{:else}<Mic size={16} />{/if}
        </button>
        <button
          class="ctl-dd"
          class:open={audioMenu === 'input'}
          aria-label={t('chat.call.microphone')}
          title={t('chat.call.microphone')}
          onclick={() => toggleAudioMenu('input')}
        >
          <ChevronDown size={12} />
        </button>
        {#if audioMenu === 'input'}
          {@render deviceMenu(
            'input',
            audioDevices?.inputs ?? [],
            audioDevices?.selectedInput ?? null
          )}
        {/if}
      </div>

      <div class="ctl-grp" class:on={callState.deafened}>
        <button
          class="ctl-tgl"
          aria-pressed={callState.deafened}
          aria-label={callState.deafened ? t('chat.call.undeafen') : t('chat.call.deafen')}
          title={callState.deafened ? t('chat.call.undeafen') : t('chat.call.deafen')}
          onclick={() => callState.toggleDeafen()}
        >
          {#if callState.deafened}<HeadphoneOff size={16} />{:else}<Headphones size={16} />{/if}
        </button>
        <button
          class="ctl-dd"
          class:open={audioMenu === 'output'}
          aria-label={t('chat.call.output')}
          title={t('chat.call.output')}
          onclick={() => toggleAudioMenu('output')}
        >
          <ChevronDown size={12} />
        </button>
        {#if audioMenu === 'output'}
          {@render deviceMenu(
            'output',
            audioDevices?.outputs ?? [],
            audioDevices?.selectedOutput ?? null
          )}
        {/if}
      </div>
    </div>
  </div>
</aside>

{#snippet deviceMenu(kind: 'input' | 'output', items: string[], selected: string | null)}
  <div class="ctl-menu">
    <button class="ctl-opt" class:sel={selected === null} onclick={() => pickDevice(kind, null)}>
      <span>{t('chat.call.systemDefault')}</span>
      {#if selected === null}<Check size={13} />{/if}
    </button>
    {#each items as name}
      <button class="ctl-opt" class:sel={selected === name} onclick={() => pickDevice(kind, name)}>
        <span>{name}</span>
        {#if selected === name}<Check size={13} />{/if}
      </button>
    {/each}
    {#if items.length === 0}
      <div class="ctl-empty">{t('chat.call.noDevices')}</div>
    {/if}
  </div>
{/snippet}

{#if contactMenu}
  {@const menu = contactMenu}
  <div
    class="ctx-backdrop"
    onclick={() => (contactMenu = null)}
    oncontextmenu={(e) => {
      e.preventDefault();
      contactMenu = null;
    }}
    role="presentation"
  ></div>
  <div class="ctx-menu" style="left: {menu.x}px; top: {menu.y}px;">
    <button
      class="ctx-item"
      onclick={() => {
        pinnedConvosState.toggle(menu.userId);
        contactMenu = null;
      }}
    >
      <Pin size={14} />
      {pinnedConvosState.has(menu.userId)
        ? t('layout.contactMenu.unpin')
        : t('layout.contactMenu.pin')}
    </button>
    {#if messagesState.unreadFor(menu.userId) > 0}
      <button
        class="ctx-item"
        onclick={() => {
          messagesState.markRead(menu.userId, true);
          contactMenu = null;
        }}
      >
        <Check size={14} />
        {t('layout.contactMenu.markRead')}
      </button>
    {:else}
      <button
        class="ctx-item"
        onclick={() => {
          messagesState.markUnread(menu.userId);
          contactMenu = null;
        }}
      >
        <Mail size={14} />
        {t('layout.contactMenu.markUnread')}
      </button>
    {/if}
    <button class="ctx-item" onclick={() => startRename(menu.userId)}>
      <Pencil size={14} />
      {t('layout.contactMenu.rename')}
    </button>
    <button
      class="ctx-item"
      onclick={() => {
        void contactsState
          .setMuted(menu.userId, !contactsState.isMuted(menu.userId))
          .catch(() => {});
        contactMenu = null;
      }}
    >
      {#if contactsState.isMuted(menu.userId)}
        <Bell size={14} />
        {t('layout.contactMenu.unmute')}
      {:else}
        <BellOff size={14} />
        {t('layout.contactMenu.mute')}
      {/if}
    </button>
    <button
      class="ctx-item"
      onclick={() => {
        archivedConvosState.toggle(menu.userId);
        contactMenu = null;
      }}
    >
      <Archive size={14} />
      {archivedConvosState.has(menu.userId)
        ? t('layout.contactMenu.unarchive')
        : t('layout.contactMenu.archive')}
    </button>
    <button class="ctx-item danger" onclick={() => handleToggleBlock(menu.userId)}>
      <Ban size={14} />
      {contactsState.getById(menu.userId)?.blocked
        ? t('layout.contactMenu.unblock')
        : t('layout.contactMenu.block')}
    </button>
    <button class="ctx-item danger" onclick={() => handleDeleteContact(menu.userId)}>
      <Trash2 size={14} />
      {t('layout.contactMenu.delete')}
    </button>
  </div>
{/if}

<style>
  .sidebar {
    width: var(--sidebar-width);
    padding-left: var(--safe-left);
    padding-bottom: var(--safe-bottom);
    background: var(--panel);
    backdrop-filter: blur(24px) saturate(140%);
    -webkit-backdrop-filter: blur(24px) saturate(140%);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    box-shadow: inset -1px 0 0 var(--panel-border);
    overflow: hidden;
  }

  .sidebar-header {
    height: calc(var(--header-height) + var(--safe-top));
    padding: var(--safe-top) 12px 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  :global(html.mac) .sidebar-header {
    padding-left: 78px;
  }

  .sidebar-header :global(button) {
    -webkit-app-region: no-drag;
  }

  .sidebar-header h1 {
    font-size: var(--text-lg);
    font-weight: 700;
    letter-spacing: -0.02em;
  }
  .brand {
    -webkit-app-region: no-drag;
    text-align: left;
  }
  .brand:hover .prompt {
    color: var(--accent-hover);
  }
  .prompt {
    color: var(--accent);
    font-weight: 600;
    font-family: var(--font-mono);
  }

  .header-actions {
    display: flex;
    gap: 2px;
  }

  .icon-btn {
    width: 34px;
    height: 34px;
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    transition: all var(--transition);
  }
  .icon-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .icon-btn:active {
    transform: scale(0.95);
  }
  .icon-btn.mobile-close {
    display: none;
  }

  /* Search trigger (opens command palette) */
  .search-trigger {
    display: flex;
    align-items: center;
    gap: 6px;
    width: calc(100% - 20px);
    margin: 8px 10px 4px;
    padding: 7px 10px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text-muted);
    transition:
      border-color var(--transition),
      background var(--transition);
    text-align: left;
  }
  .search-trigger:hover {
    border-color: var(--accent-selected);
    background: var(--bg-hover);
  }
  .search-trigger-text {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .search-kbd {
    flex-shrink: 0;
    font-family: var(--font-mono);
    font-size: 10.5px;
    font-weight: 600;
    color: var(--text-muted);
    background: var(--bg-tertiary);
    border: 1px solid var(--border-light);
    border-radius: var(--radius-sm);
    padding: 1px 5px;
    line-height: 1.4;
  }

  /* Contact list */
  .contacts-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px 8px 8px;
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 24px 12px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .skeleton-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 2px 0;
  }
  .skeleton-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    opacity: 0;
    animation: sk-fade 0.4s ease forwards;
  }
  @keyframes sk-fade {
    to {
      opacity: 1;
    }
  }
  .sk-avatar {
    width: 42px;
    height: 42px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .sk-meta {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 7px;
  }
  .sk-line {
    height: 9px;
    border-radius: 999px;
  }
  .sk-name {
    width: 55%;
  }
  .sk-preview {
    width: 80%;
    height: 8px;
  }
  .sk-avatar,
  .sk-line {
    background: linear-gradient(
      90deg,
      var(--bg-hover) 25%,
      color-mix(in srgb, var(--bg-hover) 50%, var(--text-muted) 12%) 37%,
      var(--bg-hover) 63%
    );
    background-size: 400% 100%;
    animation: sk-shimmer 1.4s ease-in-out infinite;
  }
  @keyframes sk-shimmer {
    0% {
      background-position: 100% 0;
    }
    100% {
      background-position: 0 0;
    }
  }
  .empty-state.column {
    flex-direction: column;
    padding: 40px 16px;
    text-align: center;
  }
  .empty-glyph {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 56px;
    border-radius: 50%;
    color: var(--accent);
    margin-bottom: 4px;
  }
  .empty-glyph::after {
    content: '';
    position: absolute;
    inset: -8px;
    border-radius: 50%;
    background: radial-gradient(circle, var(--accent-dim), transparent 70%);
    z-index: -1;
  }
  .empty-cta {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    padding: 8px 14px;
    border-radius: 999px;
    background: var(--accent-dim);
    color: var(--accent-hover);
    font-size: 12px;
    font-weight: 600;
    transition: background var(--transition);
  }
  .empty-cta:hover {
    background: color-mix(in srgb, var(--accent) 22%, transparent);
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
  .contact-row:hover {
    background: var(--bg-hover);
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
  .contact-row.active .contact-avatar :global(.status-dot) {
    border-color: var(--bg-secondary);
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
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .contact-time {
    font-size: 11px;
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
    font-size: 11px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .badge.total {
    position: absolute;
    top: -4px;
    right: -5px;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    font-size: 10px;
    border: 2px solid var(--bg-secondary);
  }

  /* User panel */
  .user-panel {
    width: 100%;
    min-height: 49px;
    padding: 0 8px 0 0;
    border-top: 1px solid var(--border);
    display: flex;
    align-items: center;
    flex-shrink: 0;
    transition: background var(--transition);
  }
  /* Full-bleed: hovering the identity button tints the entire footer row,
     border to border, so the highlight also runs behind the call controls.
     The panel paints it (not the button) because the button only spans the
     left part of the row. */
  .user-panel:has(.user-identity:hover) {
    background: var(--bg-hover);
  }
  .user-identity {
    flex: 1;
    min-width: 0;
    align-self: stretch;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 8px 6px 12px;
    background: transparent;
    text-align: left;
  }

  .self-controls {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .ctl-grp {
    position: relative;
    display: flex;
    align-items: stretch;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
  }
  .ctl-grp.on {
    background: var(--accent-dim);
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
  }
  .ctl-tgl {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 32px;
    color: var(--text-secondary);
    transition: color var(--transition);
  }
  .ctl-grp.on .ctl-tgl {
    color: var(--accent-hover);
  }
  .ctl-tgl:hover {
    color: var(--text-primary);
  }
  .ctl-dd {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    color: var(--text-muted);
    border-left: 1px solid var(--border);
    transition:
      color var(--transition),
      background var(--transition);
  }
  .ctl-dd:hover,
  .ctl-dd.open {
    color: var(--text-primary);
    background: var(--bg-hover);
  }
  .ctl-menu {
    position: absolute;
    bottom: calc(100% + 6px);
    right: 0;
    min-width: 180px;
    max-width: 240px;
    max-height: 220px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding: 4px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-3);
    z-index: 30;
  }
  .ctl-opt {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    text-align: left;
    transition: background var(--transition);
  }
  .ctl-opt:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .ctl-opt.sel {
    color: var(--accent-hover);
  }
  .ctl-opt span {
    font-size: var(--text-2xs);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .ctl-empty {
    padding: 6px 8px;
    font-size: var(--text-2xs);
    color: var(--text-muted);
  }
  .user-avatar {
    position: relative;
    flex-shrink: 0;
  }
  .user-info {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .user-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .user-id {
    font-size: 11px;
    color: var(--text-muted);
    font-family: var(--font-mono);
  }

  .sync-slot {
    min-width: 0;
  }
  .sync-slot :global(.offline-sync) {
    margin: 4px 14px 6px;
  }

  @media (max-width: 768px) {
    .dock-slot {
      display: none;
    }
    .sidebar {
      position: fixed;
      top: 0;
      left: 0;
      bottom: 0;
      width: 300px;
      max-width: 86vw;
      z-index: 60;
      transform: translateX(-100%);
      transition:
        transform 0.26s cubic-bezier(0.4, 0, 0.2, 1),
        box-shadow 0.26s cubic-bezier(0.4, 0, 0.2, 1);
      box-shadow: none;
    }
    .sidebar.open,
    .sidebar.dragging {
      box-shadow: 4px 0 24px rgba(0, 0, 0, 0.45);
    }
    .sidebar.open {
      transform: translateX(0);
    }
    .sidebar.dragging {
      transition: none;
    }

    .icon-btn.mobile-close {
      display: flex;
    }

    .contact-row {
      padding: 11px 10px;
    }
    /* Row actions are hover-only; on touch use long-press / contextmenu. */
    .row-menu-btn {
      display: none;
    }
    .icon-btn {
      width: 38px;
      height: 38px;
    }
  }

  @media (max-width: 960px) and (min-width: 769px) {
    .sidebar {
      width: 240px;
    }
  }

  .convo-pin {
    display: inline-flex;
    align-items: center;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .convo-muted {
    display: inline-flex;
    align-items: center;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .archived-header {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 10px 14px;
    margin-top: 6px;
    border-top: 1px solid var(--border-light);
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 600;
    transition: color var(--transition);
  }
  .archived-header:hover {
    color: var(--text-secondary);
  }
  .contact-row.pinned-convo {
    background: color-mix(in srgb, var(--accent) 5%, transparent);
  }
  .contact-row-wrap {
    position: relative;
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
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.2);
    opacity: 0;
    pointer-events: none;
    transition:
      opacity var(--transition),
      background var(--transition);
  }
  @media (hover: hover) {
    .contact-row-wrap:hover .row-menu-btn {
      opacity: 1;
      pointer-events: auto;
    }
    .contact-row-wrap:hover .contact-top,
    .contact-row-wrap:hover .contact-bottom {
      padding-right: 28px;
    }
  }
  .row-menu-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .ctx-backdrop {
    position: fixed;
    inset: 0;
    z-index: 2050;
  }
  .ctx-menu {
    position: fixed;
    z-index: 2051;
    min-width: 184px;
    padding: 4px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-3);
    animation: ctx-in 120ms cubic-bezier(0.2, 0.9, 0.3, 1.1);
  }
  @keyframes ctx-in {
    from {
      opacity: 0;
      transform: scale(0.96);
    }
  }
  .ctx-item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 9px 10px;
    border-radius: var(--radius-sm);
    font-size: 13px;
    color: var(--text-primary);
    text-align: left;
    transition: background var(--transition);
  }
  .ctx-item:hover {
    background: var(--bg-hover);
  }
  .ctx-item.danger {
    color: var(--danger);
  }
  .ctx-item.danger:hover {
    background: var(--danger-dim);
  }
</style>
