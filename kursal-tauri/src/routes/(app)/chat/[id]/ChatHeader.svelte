<script lang="ts">
  import { Menu, ShieldAlert, Search, Radio, Phone } from 'lucide-svelte';
  import Avatar from '$lib/components/Avatar.svelte';
  import OfflineSyncIndicator from '$lib/components/OfflineSyncIndicator.svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { callState } from '$lib/state/call.svelte';
  import { uiState } from '$lib/state/ui.svelte';
  import type { ContactResponse } from '$lib/types';
  import { t } from '$lib/i18n';
  import { connectionLabel } from '$lib/utils/connectionLabel';

  interface Props {
    contact: ContactResponse;
    onOpenProfile: () => void;
    onOpenSecurity: () => void;
    onOpenConnection: () => void;
    onSearch: () => void;
  }

  let { contact, onOpenProfile, onOpenSecurity, onOpenConnection, onSearch }: Props = $props();

  const status = $derived(contactsState.connectionStatus[contact.userId] ?? 'disconnected');

  const statusLabel = $derived(connectionLabel(status, contactsState.lastSeenAt(contact.userId)));
</script>

<header class="chat-header" data-tauri-drag-region="deep">
  <div class="header-left">
    <button
      class="menu-btn"
      onclick={() => (uiState.mobileSidebarOpen = true)}
      aria-label={t('chat.header.openSidebar')}
    >
      <Menu size={20} />
    </button>
    <button
      class="header-profile"
      onclick={onOpenProfile}
      aria-label={t('chat.header.viewProfile')}
    >
      <Avatar name={contact.displayName} src={contact.avatarPath} size={34} {status} showStatus />
      <div class="header-info">
        <div class="header-name-row">
          <span class="header-name">{contact.displayName}</span>
          {#if contactsState.aliasFor(contact.userId) && contact.profileName && contact.profileName !== contact.displayName}
            <span class="header-real-name">~ {contact.profileName}</span>
          {/if}
        </div>
        <span class="header-meta">
          <span class="header-status" data-status={status}>{statusLabel}</span>
          <span class="header-sync"><OfflineSyncIndicator compact /></span>
        </span>
      </div>
    </button>
  </div>
  <div class="header-right">
    <button
      class="icon-btn"
      onclick={() => callState.start(contact.userId)}
      disabled={callState.status !== 'idle'}
      title={t('chat.call.startCall')}
      aria-label={t('chat.call.startCall')}
    >
      <Phone size={17} />
    </button>
    <button class="icon-btn" onclick={onSearch} aria-label={t('chat.header.search')}>
      <Search size={17} />
    </button>
    <button
      class="icon-btn"
      onclick={onOpenConnection}
      title={t('chat.header.connectionInfo')}
      aria-label={t('chat.header.connectionInfo')}
    >
      <Radio size={17} />
    </button>
    {#if !contact.verified}
      <button
        class="verify-btn"
        title={t('chat.header.verifyIdentity')}
        onclick={onOpenSecurity}
        aria-label={t('chat.header.verifyIdentity')}
      >
        <ShieldAlert size={16} />
      </button>
    {/if}
  </div>
</header>

<style>
  .chat-header {
    height: calc(var(--header-height) + var(--safe-top));
    padding: var(--safe-top) max(12px, var(--safe-right)) 0 max(12px, var(--safe-left));
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    flex-shrink: 0;
    z-index: 10;
  }
  .header-left {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .menu-btn {
    display: none;
    width: 36px;
    height: 36px;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-md);
    color: var(--text-secondary);
    transition: background var(--transition);
    -webkit-app-region: no-drag;
  }
  @media (hover: hover) {
    .menu-btn:hover {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
  }
  .menu-btn:active {
    background: var(--bg-hover);
    transform: scale(0.96);
  }
  @media (max-width: 768px) {
    .menu-btn {
      display: flex;
    }
    :global(html.mac) .chat-header {
      padding-left: 78px;
    }
  }

  .header-profile {
    display: inline-flex;
    align-items: center;
    gap: 12px;
    padding: 4px 10px 4px 6px;
    border-radius: var(--radius-md);
    transition: background var(--transition);
    min-width: 0;
    max-width: min(320px, 60vw);
    -webkit-app-region: no-drag;
  }
  @media (hover: hover) {
    .header-profile:hover {
      background: var(--bg-hover);
    }
  }
  .header-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
    align-items: flex-start;
    gap: 1px;
  }
  .header-name-row {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
    max-width: 100%;
  }
  .header-name {
    font-size: 14.5px;
    font-weight: 600;
    line-height: 1.2;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .header-real-name {
    font-size: 11.5px;
    color: var(--text-muted);
    font-family: var(--font-mono);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 1;
    min-width: 0;
  }
  .header-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  /* Desktop reads this off the sidebar footer; the header only carries it on
     mobile, where the sidebar is a closed slide-over. */
  .header-sync {
    display: none;
  }
  @media (max-width: 768px) {
    .header-sync {
      display: inline-flex;
    }
  }
  .header-status {
    font-size: var(--text-xs);
    color: var(--text-muted);
    line-height: 1.2;
    transition: color var(--transition);
  }
  .header-status[data-status='direct'],
  .header-status[data-status='holepunch'] {
    color: var(--success);
  }
  .header-status[data-status='relay'] {
    color: var(--info);
  }
  .header-status[data-status='connecting'] {
    color: var(--warning);
  }
  .header-right {
    display: flex;
    gap: 4px;
    -webkit-app-region: no-drag;
  }
  .icon-btn {
    width: 36px;
    height: 36px;
  }
  @media (hover: hover) {
    .icon-btn:hover {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
  }
  .icon-btn:active {
    transform: scale(0.95);
  }
  .verify-btn {
    width: 36px;
    height: 36px;
    border-radius: var(--radius-md);
    background: rgba(251, 191, 36, 0.12);
    color: var(--warning);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background var(--transition);
  }
  @media (hover: hover) {
    .verify-btn:hover {
      background: rgba(251, 191, 36, 0.22);
    }
  }
  .verify-btn:active {
    transform: scale(0.95);
  }
</style>
