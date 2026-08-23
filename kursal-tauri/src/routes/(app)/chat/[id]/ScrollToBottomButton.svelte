<script lang="ts">
  import { t } from '$lib/i18n';
  import { ChevronDown } from 'lucide-svelte';
  import Avatar from '$lib/components/Avatar.svelte';

  interface Props {
    unreadCount: number;
    name: string;
    avatar: string | null | undefined;
    onClick: () => void;
  }

  let { unreadCount, name, avatar, onClick }: Props = $props();
</script>

<button
  class="scroll-to-bottom"
  class:has-unread={unreadCount > 0}
  onclick={onClick}
  aria-label={t('chat.conversation.jumpToLatest')}
>
  {#if unreadCount > 0}
    <Avatar {name} src={avatar} size={22} />
    <span class="scroll-badge">{unreadCount > 99 ? '99+' : unreadCount}</span>
  {:else}
    <ChevronDown size={18} />
  {/if}
</button>

<style>
  .scroll-to-bottom {
    position: absolute;
    right: max(calc(16px + var(--safe-right)), calc((100% - var(--chat-max)) / 2 + 16px));
    bottom: calc(var(--composer-h, 76px) + var(--safe-bottom) + 16px);
    height: 36px;
    min-width: 36px;
    border-radius: var(--radius-md);
    background: var(--surface);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid var(--border-light);
    color: var(--text-primary);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 0 4px;
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.12);
    transition:
      transform var(--transition),
      background var(--transition);
    z-index: 15;
    animation: fadeInFab 0.22s cubic-bezier(0.34, 1.56, 0.64, 1);
    font-size: var(--text-sm);
    font-weight: 600;
  }
  .scroll-to-bottom.has-unread {
    padding: 0 10px 0 4px;
  }
  @media (hover: hover) {
    .scroll-to-bottom:hover {
      background: var(--bg-hover);
      transform: translateY(-1px);
    }
  }
  .scroll-to-bottom:active {
    transform: translateY(0);
  }
  @keyframes fadeInFab {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .scroll-badge {
    background: var(--accent);
    color: #fff;
    font-size: var(--text-2xs);
    font-weight: 700;
    min-width: 18px;
    height: 18px;
    padding: 0 6px;
    border-radius: var(--radius-md);
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
</style>
