<script lang="ts">
  import { Phone, PhoneOff } from 'lucide-svelte';
  import { goto } from '$app/navigation';
  import { t } from '$lib/i18n';
  import Avatar from '$lib/components/Avatar.svelte';
  import { callState } from '$lib/state/call.svelte';
  import { contactsState } from '$lib/state/contacts.svelte';

  const contact = $derived(callState.contactId ? contactsState.getById(callState.contactId) : null);
  const visible = $derived(callState.status === 'ringing_in');

  async function accept() {
    const id = callState.contactId;
    await callState.accept();
    if (id) goto('/chat/' + id);
  }
</script>

{#if visible && contact}
  <div class="incoming" role="dialog" aria-modal="false" aria-label={t('chat.call.incoming')}>
    <span class="ava">
      <Avatar name={contact.displayName} src={contact.avatarBase64} size={40} />
    </span>
    <div class="meta">
      <span class="name">{contact.displayName}</span>
      <span class="sub">{t('chat.call.incoming')}</span>
    </div>
    <div class="actions">
      <button
        class="btn decline"
        onclick={() => callState.decline()}
        aria-label={t('chat.call.decline')}
        title={t('chat.call.decline')}
      >
        <PhoneOff size={17} />
      </button>
      <button
        class="btn accept"
        onclick={accept}
        aria-label={t('chat.call.accept')}
        title={t('chat.call.accept')}
      >
        <Phone size={17} />
      </button>
    </div>
  </div>
{/if}

<style>
  .incoming {
    position: fixed;
    top: calc(12px + var(--safe-top));
    left: var(--safe-center-x);
    transform: translateX(-50%);
    z-index: 650;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 10px 8px 8px;
    background: var(--surface);
    backdrop-filter: blur(16px) saturate(140%);
    -webkit-backdrop-filter: blur(16px) saturate(140%);
    border: 1px solid var(--border-light);
    border-radius: 999px;
    box-shadow: var(--shadow-3);
    animation: incoming-in 0.24s cubic-bezier(0.34, 1.56, 0.64, 1);
    max-width: calc(var(--safe-w) - 24px);
  }
  @keyframes incoming-in {
    from {
      opacity: 0;
      transform: translate(-50%, -10px);
    }
  }
  .ava {
    display: flex;
    border-radius: 50%;
    flex-shrink: 0;
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 55%, transparent);
    animation: ava-pulse 1.6s ease-in-out infinite;
  }
  @keyframes ava-pulse {
    0%,
    100% {
      box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 55%, transparent);
    }
    50% {
      box-shadow: 0 0 0 5px color-mix(in srgb, var(--accent) 12%, transparent);
    }
  }
  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
    max-width: 150px;
  }
  .name {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sub {
    font-size: var(--text-2xs);
    color: var(--text-muted);
  }
  .actions {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
  }
  .btn {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    transition:
      transform var(--transition),
      filter var(--transition);
  }
  .btn:active {
    transform: scale(0.92);
  }
  .btn.decline {
    background: var(--danger);
  }
  .btn.decline:hover {
    filter: brightness(1.1);
  }
  .btn.accept {
    background: var(--success);
    animation: accept-pulse 1.6s ease-in-out infinite;
  }
  .btn.accept:hover {
    filter: brightness(1.08);
  }
  @keyframes accept-pulse {
    0%,
    100% {
      box-shadow: 0 0 0 0 color-mix(in srgb, var(--success) 55%, transparent);
    }
    70% {
      box-shadow: 0 0 0 10px color-mix(in srgb, var(--success) 0%, transparent);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .ava,
    .btn.accept {
      animation: none;
    }
  }
</style>
