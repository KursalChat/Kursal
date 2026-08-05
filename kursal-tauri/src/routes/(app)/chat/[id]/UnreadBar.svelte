<script lang="ts">
  import { ArrowUp, Mail, CheckCheck } from 'lucide-svelte';
  import { t } from '$lib/i18n';

  interface Props {
    count: number;
    onJump: () => void;
    onMarkRead: () => void;
  }

  let { count, onJump, onMarkRead }: Props = $props();

  const label = $derived(
    count === 1 ? t('chat.conversation.unreadBarOne') : t('chat.conversation.unreadBar', { count })
  );
</script>

<div class="unread-bar">
  <button class="unread-main" onclick={onJump} title={t('chat.conversation.unreadBarJump')}>
    <Mail size={14} class="unread-icon" />
    <span class="unread-label">{label}</span>
    <ArrowUp size={14} class="unread-jump" />
  </button>

  <button
    class="unread-read"
    onclick={onMarkRead}
    title={t('chat.conversation.unreadBarMarkReadHint')}
  >
    <CheckCheck size={14} />
    <span>{t('chat.conversation.unreadBarMarkRead')}</span>
  </button>
</div>

<style>
  .unread-bar {
    display: flex;
    align-items: stretch;
    gap: 4px;
    padding: 5px max(8px, var(--safe-right)) 5px max(8px, var(--safe-left));
    background: var(--bg-secondary);
    backdrop-filter: blur(14px) saturate(140%);
    -webkit-backdrop-filter: blur(14px) saturate(140%);
    border-bottom: 1px solid var(--border-light);
    flex-shrink: 0;
  }
  /* Translucency only where color-mix() exists: a color-mix() value containing
     var() computes to `transparent` on engines without it (Chrome < 111). */
  @supports (background: color-mix(in srgb, red 50%, transparent)) {
    .unread-bar {
      background: color-mix(in srgb, var(--bg-secondary) 86%, transparent);
    }
  }

  .unread-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    text-align: left;
    transition: background var(--transition);
  }
  .unread-main:hover {
    background: var(--bg-hover);
  }
  .unread-main:active {
    transform: scale(0.99);
  }
  .unread-main :global(.unread-icon),
  .unread-main :global(.unread-jump) {
    color: var(--accent);
    flex-shrink: 0;
  }

  .unread-label {
    flex: 1;
    min-width: 0;
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--accent-hover);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .unread-read {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 4px 9px;
    border-radius: var(--radius-sm);
    font-size: var(--text-2xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--text-muted);
    transition: all var(--transition);
  }
  .unread-read:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .unread-read:active {
    transform: scale(0.97);
  }

  @media (prefers-reduced-motion: no-preference) {
    .unread-bar {
      animation: unread-bar-in 220ms cubic-bezier(0.2, 0.9, 0.3, 1.1);
    }
  }
  @keyframes unread-bar-in {
    from {
      opacity: 0;
      transform: translateY(-100%);
    }
  }
</style>
