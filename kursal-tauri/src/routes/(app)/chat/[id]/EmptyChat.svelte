<script lang="ts">
  import { t } from '$lib/i18n';
  import { ShieldAlert } from 'lucide-svelte';
  import Avatar from '$lib/components/Avatar.svelte';

  interface Props {
    name: string;
    avatar: string | null | undefined;
    verified: boolean;
    sending: boolean;
    canMessage: boolean;
    onSayHi: () => void;
    onVerify: () => void;
  }

  let { name, avatar, verified, sending, canMessage, onSayHi, onVerify }: Props = $props();
</script>

<div class="empty-chat">
  <div class="empty-avatar">
    <span class="empty-avatar-glow"></span>
    <Avatar {name} src={avatar} size={84} />
  </div>
  <h3>{t('chat.conversation.emptyHeading', { name })}</h3>
  <p class="empty-trust">
    <ShieldAlert size={13} />
    {t('chat.conversation.emptyEncrypted')}
  </p>
  {#if canMessage}
    <button class="empty-hi" disabled={sending} onclick={onSayHi}>
      <span class="empty-hi-wave">👋</span>
      {t('chat.conversation.sayHi', { name })}
    </button>
  {/if}
  {#if !verified}
    <button class="empty-verify" onclick={onVerify}>
      <ShieldAlert size={14} />
      {t('chat.conversation.verifyButton')}
    </button>
  {/if}
</div>

<style>
  .empty-chat {
    margin: auto;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    color: var(--text-muted);
    padding: 40px 24px;
    animation: empty-in 0.4s cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  @keyframes empty-in {
    from {
      opacity: 0;
      transform: translateY(12px) scale(0.97);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }
  .empty-avatar {
    position: relative;
    display: inline-flex;
    margin-bottom: 4px;
  }
  .empty-avatar-glow {
    position: absolute;
    inset: -16px;
    border-radius: 50%;
    background: radial-gradient(
      circle,
      color-mix(in srgb, var(--accent) 35%, transparent) 0%,
      transparent 70%
    );
    animation: empty-breathe 3.4s ease-in-out infinite;
    pointer-events: none;
  }
  @keyframes empty-breathe {
    0%,
    100% {
      opacity: 0.7;
      transform: scale(1);
    }
    50% {
      opacity: 1;
      transform: scale(1.08);
    }
  }
  .empty-chat h3 {
    font-size: 22px;
    color: var(--text-primary);
    margin: 4px 0 0;
    font-weight: 600;
    letter-spacing: -0.01em;
  }
  .empty-trust {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text-muted);
    margin: 0;
  }
  .empty-verify {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    padding: 8px 14px;
    background: rgba(251, 191, 36, 0.12);
    color: var(--warning);
    border-radius: var(--radius-md);
    font-size: 12px;
    font-weight: 600;
    transition: background var(--transition);
  }
  .empty-verify:hover {
    background: rgba(251, 191, 36, 0.22);
  }
  .empty-hi {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 14px;
    padding: 9px 18px;
    background: var(--accent-dim);
    color: var(--accent);
    border-radius: var(--radius-md);
    font-size: 13px;
    font-weight: 700;
    transition:
      background var(--transition),
      transform var(--transition);
  }
  .empty-hi:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 24%, transparent);
    transform: translateY(-1px);
  }
  .empty-hi:disabled {
    opacity: 0.6;
  }
  .empty-hi-wave {
    font-size: 16px;
    display: inline-block;
  }
  .empty-hi:hover:not(:disabled) .empty-hi-wave {
    animation: wave 0.7s ease-in-out;
  }
  @keyframes wave {
    0%,
    100% {
      transform: rotate(0);
    }
    25% {
      transform: rotate(18deg);
    }
    60% {
      transform: rotate(-12deg);
    }
  }
</style>
