<script lang="ts">
  import { goto } from '$app/navigation';
  import { log } from '$lib/utils/log';
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { listen } from '@tauri-apps/api/event';
  import { Menu } from 'lucide-svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { messagesState } from '$lib/state/messages.svelte';
  import { profileState } from '$lib/state/profile.svelte';
  import { networkState } from '$lib/state/network.svelte';
  import { pinnedConvosState } from '$lib/state/pinnedConvos.svelte';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import WinstonContactsTour from '$lib/components/WinstonContactsTour.svelte';
  import AutostartPrompt from '$lib/components/AutostartPrompt.svelte';
  import WinstonTip from '$lib/components/WinstonTip.svelte';
  import CommandPalette from '$lib/components/CommandPalette.svelte';
  import ShortcutsHelp from '$lib/components/ShortcutsHelp.svelte';
  import CallOverlay from '$lib/components/CallOverlay.svelte';
  import CallIncoming from '$lib/components/CallIncoming.svelte';
  import CallPill from '$lib/components/CallPill.svelte';
  import { callState } from '$lib/state/call.svelte';
  import { setBadgeCount } from '$lib/api/window';
  import { uiState } from '$lib/state/ui.svelte';
  import { pendingDropState, contactDropTargetAt } from '$lib/state/pendingDrop.svelte';
  import type { PeerIdHolderPayload } from '$lib/types';
  import { t } from '$lib/i18n';

  let { children } = $props();

  const totalUnread = $derived(messagesState.totalUnread());

  const isOffline = $derived(networkState.initialized && !networkState.online);

  $effect(() => {
    const count = totalUnread;
    (async () => {
      await setBadgeCount(count);
    })();
  });

  onMount(() => {
    void callState.init();
    let unlisten: (() => void) | null = null;
    let unlistenDrop: (() => void) | null = null;
    let disposed = false;
    (async () => {
      await profileState.load();

      try {
        const fn = await listen<PeerIdHolderPayload>('contact_removed', (e) => {
          const removedId = e.payload.peerId;
          contactsState.remove(removedId);
          if (currentChatId === removedId) {
            goto('/chat', { replaceState: true });
          }
        });
        if (disposed) fn();
        else unlisten = fn;
      } catch (e) {
        log.error('Failed to set up listeners:', e);
      }

      // File drops aimed at a sidebar contact row route to that chat; drops
      // anywhere else are handled by the open chat's own listener.
      try {
        const { getCurrentWebview } = await import('@tauri-apps/api/webview');
        const fn = await getCurrentWebview().onDragDropEvent((event) => {
          const p = event.payload;
          if (p.type === 'enter' || p.type === 'over') {
            pendingDropState.setHover(contactDropTargetAt(p.position));
          } else if (p.type === 'leave') {
            pendingDropState.setHover(null);
          } else if (p.type === 'drop') {
            pendingDropState.setHover(null);
            const target = contactDropTargetAt(p.position);
            const paths = (p as { paths?: string[] }).paths ?? [];
            if (target && paths.length) {
              pendingDropState.set(target, paths);
              goto('/chat/' + target);
            }
          }
        });
        if (disposed) fn();
        else unlistenDrop = fn;
      } catch (e) {
        log.warn('Sidebar drop routing unavailable:', e);
      }
    })();
    return () => {
      disposed = true;
      unlisten?.();
      unlistenDrop?.();
    };
  });

  const currentChatId = $derived(page.params.id);

  const sortedContacts = $derived.by(() => {
    const list = [...contactsState.contacts];
    list.sort((a, b) => {
      const ap = pinnedConvosState.has(a.userId) ? 1 : 0;
      const bp = pinnedConvosState.has(b.userId) ? 1 : 0;
      if (ap !== bp) return bp - ap;
      const aMsgs = messagesState.forContact(a.userId);
      const bMsgs = messagesState.forContact(b.userId);
      const aTs = aMsgs.length ? aMsgs[aMsgs.length - 1].timestamp : a.createdAt * 1000;
      const bTs = bMsgs.length ? bMsgs[bMsgs.length - 1].timestamp : b.createdAt * 1000;
      return bTs - aTs;
    });
    return list;
  });

  let commandOpen = $state(false);
  let helpOpen = $state(false);

  function switchConversation(dir: number, unreadOnly: boolean) {
    let list = sortedContacts;
    if (unreadOnly) list = list.filter((c) => messagesState.unreadFor(c.userId) > 0);
    if (list.length === 0) return;
    const idx = list.findIndex((c) => c.userId === currentChatId);
    const nextIdx =
      idx === -1 ? (dir > 0 ? 0 : list.length - 1) : (idx + dir + list.length) % list.length;
    const next = list[nextIdx];
    if (next) {
      uiState.mobileSidebarOpen = false;
      goto('/chat/' + next.userId);
    }
  }

  function onGlobalKey(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey;

    if (mod && !e.altKey && e.key.toLowerCase() === 'k') {
      if (e.shiftKey) return;
      e.preventDefault();
      commandOpen = !commandOpen;
      return;
    }
    if (mod && e.key === '/') {
      e.preventDefault();
      helpOpen = !helpOpen;
      return;
    }
    if (commandOpen || helpOpen) return;

    if (e.altKey && !mod) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        switchConversation(1, e.shiftKey);
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        switchConversation(-1, e.shiftKey);
      } else if (!e.shiftKey && /^Digit[1-9]$/.test(e.code)) {
        e.preventDefault();
        const c = sortedContacts[Number(e.code.slice(5)) - 1];
        if (c) {
          uiState.mobileSidebarOpen = false;
          goto('/chat/' + c.userId);
        }
      }
    }
  }

  $effect(() => {
    window.addEventListener('keydown', onGlobalKey);
    return () => window.removeEventListener('keydown', onGlobalKey);
  });
</script>

<div class="shell" class:chat-active={!!currentChatId}>
  {#if uiState.mobileSidebarOpen}
    <div
      class="backdrop"
      onclick={() => (uiState.mobileSidebarOpen = false)}
      aria-hidden="true"
    ></div>
  {/if}

  <div class="mobile-bar" class:hidden={!!currentChatId} data-tauri-drag-region>
    <button
      class="mobile-menu"
      onclick={() => (uiState.mobileSidebarOpen = true)}
      aria-label={t('layout.openMenu')}
    >
      <Menu size={22} />
    </button>
    <button class="mobile-title" onclick={() => goto('/')}>
      {t('layout.appName')}<span class="prompt">:~$</span>
    </button>
  </div>

  <Sidebar contacts={sortedContacts} {currentChatId} onOpenCommand={() => (commandOpen = true)} />

  <main class="content" class:offline={isOffline}>
    {#if isOffline}
      <div class="offline-banner" role="status">
        <span class="offline-dot"></span>
        <span>{t('layout.bannerOffline')}</span>
      </div>
    {/if}
    {@render children()}
  </main>
</div>

<WinstonContactsTour />
<WinstonTip />
<AutostartPrompt />
<CommandPalette bind:open={commandOpen} onClose={() => (commandOpen = false)} />
<ShortcutsHelp bind:open={helpOpen} onClose={() => (helpOpen = false)} />
<CallOverlay />
<CallIncoming />
<CallPill />

<style>
  .shell {
    display: flex;
    height: 100%;
    overflow: hidden;
  }

  .offline-banner {
    position: absolute;
    top: var(--safe-top);
    left: 0;
    right: 0;
    z-index: 50;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 6px 12px;
    background: var(--warning, #f59e0b);
    color: #1a1300;
    font-size: 12.5px;
    font-weight: 600;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.18);
    pointer-events: none;
    animation: offline-banner-in 220ms ease;
  }
  .offline-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #1a1300;
    animation: offline-dot-pulse 1.6s ease-in-out infinite;
  }
  @keyframes offline-banner-in {
    from {
      transform: translateY(-100%);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }
  @keyframes offline-dot-pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.4;
    }
  }

  .backdrop {
    display: none;
  }
  .mobile-bar {
    display: none;
  }

  .mobile-bar :global(button),
  .mobile-title {
    -webkit-app-region: no-drag;
  }

  .prompt {
    color: var(--accent);
    font-weight: 600;
    font-family: var(--font-mono);
  }

  /* Main content */
  .content {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    background: transparent;
    position: relative;
  }
  /* Offline banner is absolutely positioned at the top; reserve space so it
   * never overlaps the page header beneath it. ~30px banner body. */
  .content.offline {
    padding-top: calc(30px + var(--safe-top));
  }

  /* Mobile responsive */
  @media (max-width: 768px) {
    .backdrop {
      display: block;
      position: fixed;
      inset: 0;
      background: rgba(0, 0, 0, 0.6);
      z-index: 50;
      animation: fadeIn 0.2s ease;
    }
    @keyframes fadeIn {
      from {
        opacity: 0;
      }
      to {
        opacity: 1;
      }
    }

    .mobile-bar {
      display: flex;
      align-items: center;
      gap: 8px;
      position: fixed;
      top: 0;
      left: 0;
      right: 0;
      height: calc(var(--header-height) + var(--safe-top));
      padding: var(--safe-top) 6px 0;
      background: var(--panel);
      backdrop-filter: blur(20px) saturate(140%);
      -webkit-backdrop-filter: blur(20px) saturate(140%);
      border-bottom: 1px solid var(--border);
      z-index: 30;
    }
    .mobile-bar.hidden {
      display: none;
    }
    .mobile-menu {
      width: 40px;
      height: 40px;
      display: flex;
      align-items: center;
      justify-content: center;
      color: var(--text-primary);
      border-radius: var(--radius-md);
    }
    .mobile-menu:active {
      background: var(--bg-hover);
    }
    .mobile-title {
      flex: 1;
      text-align: center;
      font-size: 16px;
      font-weight: 700;
    }

    .content {
      width: 100%;
    }
    .shell:not(.chat-active) .content {
      padding-top: calc(var(--header-height) + var(--safe-top));
    }
    .shell:not(.chat-active) .content.offline {
      padding-top: calc(var(--header-height) + 30px + var(--safe-top));
    }
    .shell:not(.chat-active) .offline-banner {
      top: calc(var(--header-height) + var(--safe-top));
    }
  }
</style>
