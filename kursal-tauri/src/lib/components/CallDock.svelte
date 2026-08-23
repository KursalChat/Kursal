<script lang="ts">
  import { MicOff, HeadphoneOff, PhoneOff, ChevronUp } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import Avatar from '$lib/components/Avatar.svelte';
  import { callState } from '$lib/state/call.svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { createCallElapsed } from '$lib/utils/callElapsed.svelte';

  const contact = $derived(callState.contactId ? contactsState.getById(callState.contactId) : null);
  const phase = $derived(callState.status);
  const visible = $derived(
    phase === 'ringing_out' || phase === 'connecting' || phase === 'connected'
  );
  const isConnected = $derived(phase === 'connected');

  const timer = createCallElapsed(
    () => isConnected,
    () => callState.startedAt
  );

  const label = $derived(
    isConnected
      ? timer.label
      : phase === 'ringing_out'
        ? t('chat.call.ringing')
        : t('chat.call.connecting')
  );
</script>

{#if visible && contact}
  <div class="dock">
    <div class="banner">
      <button
        class="grab"
        onclick={() => callState.maximize()}
        aria-label={t('chat.call.expand')}
        title={t('chat.call.expand')}
      >
        <span class="ava" class:live={isConnected && callState.remoteLevel > 0.05}>
          <Avatar name={contact.displayName} src={contact.avatarPath} size={26} />
          {#if isConnected && callState.peerDeafened}
            <span class="vs-badge" title={t('chat.call.peerDeafened')}>
              <HeadphoneOff size={10} />
            </span>
          {:else if isConnected && callState.peerMuted}
            <span class="vs-badge" title={t('chat.call.peerMuted')}>
              <MicOff size={10} />
            </span>
          {/if}
        </span>
        <span class="meta">
          <span class="name">{contact.displayName}</span>
          <span class="time">{label}</span>
        </span>
        <ChevronUp size={14} />
      </button>
      <button
        class="hangup"
        onclick={() => callState.end()}
        aria-label={t('chat.call.hangup')}
        title={t('chat.call.hangup')}
      >
        <PhoneOff size={15} />
      </button>
    </div>
  </div>
{/if}

<style>
  .dock {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    margin: 0 8px 6px;
    background: var(--surface-soft);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
  }
  .banner {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .grab {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    padding: 4px;
    border-radius: var(--radius-md);
    transition: background var(--transition);
  }
  @media (hover: hover) {
    .grab:hover {
      background: var(--bg-hover);
    }
  }
  .ava {
    position: relative;
    display: flex;
    border-radius: 50%;
    flex-shrink: 0;
    transition: box-shadow 0.12s linear;
  }
  .ava.live {
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--success) 60%, transparent);
  }
  .vs-badge {
    position: absolute;
    right: -3px;
    bottom: -3px;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    color: #fff;
    background: var(--danger);
    border: 1.5px solid var(--surface-soft);
  }
  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
    text-align: left;
  }
  .name {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .time {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    color: var(--success);
    font-variant-numeric: tabular-nums;
  }
  .hangup {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: var(--radius-md);
    color: #fff;
    background: var(--danger);
    flex-shrink: 0;
    transition: filter var(--transition);
  }
  @media (hover: hover) {
    .hangup:hover {
      filter: brightness(1.1);
    }
  }
</style>
