<script lang="ts">
  import { Mic, MicOff, PhoneOff, ChevronUp } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import Avatar from '$lib/components/Avatar.svelte';
  import { callState } from '$lib/state/call.svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { createCallElapsed } from '$lib/utils/callElapsed.svelte';

  const contact = $derived(callState.contactId ? contactsState.getById(callState.contactId) : null);

  const phase = $derived(callState.status);
  const visible = $derived(
    !callState.expanded &&
      (phase === 'ringing_out' || phase === 'connecting' || phase === 'connected')
  );

  const timer = createCallElapsed(
    () => phase === 'connected',
    () => callState.startedAt
  );

  const label = $derived(
    phase === 'connected'
      ? timer.label
      : phase === 'ringing_out'
        ? t('chat.call.ringing')
        : t('chat.call.connecting')
  );

  const live = $derived(phase === 'connected' && callState.remoteLevel > 0.05);
</script>

{#if visible && contact}
  <div class="pill">
    <button class="grab" onclick={() => callState.maximize()} aria-label={t('chat.call.expand')}>
      <span class="ava" class:live>
        <Avatar name={contact.displayName} src={contact.avatarBase64} size={28} />
      </span>
      <span class="info">
        <span class="name">{contact.displayName}</span>
        <span class="time">{label}</span>
      </span>
      <ChevronUp size={14} class="chev" />
    </button>
    <div class="quick">
      <button
        class="mini"
        class:active={callState.muted}
        aria-pressed={callState.muted}
        onclick={() => callState.toggleMute()}
        aria-label={callState.muted ? t('chat.call.unmute') : t('chat.call.mute')}
      >
        {#if callState.muted}<MicOff size={15} />{:else}<Mic size={15} />{/if}
      </button>
      <button
        class="mini hangup"
        onclick={() => callState.end()}
        aria-label={t('chat.call.hangup')}
      >
        <PhoneOff size={15} />
      </button>
    </div>
  </div>
{/if}

<style>
  .pill {
    position: fixed;
    top: calc(10px + var(--safe-top));
    left: var(--safe-center-x);
    transform: translateX(-50%);
    z-index: 500;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px 6px 6px;
    background: var(--surface);
    backdrop-filter: blur(16px) saturate(140%);
    -webkit-backdrop-filter: blur(16px) saturate(140%);
    border: 1px solid var(--border-light);
    border-radius: 999px;
    box-shadow: var(--shadow-2);
    animation: pill-in 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  @keyframes pill-in {
    from {
      opacity: 0;
      transform: translate(-50%, -8px);
    }
  }
  .grab {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 2px 4px 2px 2px;
    border-radius: 999px;
    transition: background var(--transition);
  }
  .grab:hover {
    background: var(--bg-hover);
  }
  .ava {
    display: flex;
    border-radius: 50%;
    transition: box-shadow 0.12s linear;
  }
  .ava.live {
    box-shadow: 0 0 0 2px var(--success);
  }
  .info {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0;
    min-width: 0;
    max-width: 130px;
  }
  .name {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-primary);
    line-height: 1.25;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }
  .time {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    color: var(--accent-hover);
    line-height: 1.2;
    font-variant-numeric: tabular-nums;
  }
  :global(.pill .chev) {
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .quick {
    display: flex;
    gap: 4px;
  }
  .mini {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    background: var(--bg-hover);
    transition:
      background var(--transition),
      color var(--transition),
      transform var(--transition);
  }
  .mini:hover {
    color: var(--text-primary);
  }
  .mini:active {
    transform: scale(0.92);
  }
  .mini.active {
    background: var(--accent-dim);
    color: var(--accent-hover);
  }
  .mini.hangup {
    background: var(--danger);
    color: #fff;
  }
  .mini.hangup:hover {
    filter: brightness(1.1);
  }

  @media (min-width: 769px) {
    .pill {
      display: none;
    }
  }
</style>
