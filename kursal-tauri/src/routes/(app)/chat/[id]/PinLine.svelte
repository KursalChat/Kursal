<script lang="ts">
  import { Pin, PinOff } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import { formatTime } from './chat-utils';

  interface Props {
    pinned: boolean;
    direction: string;
    peerName: string;
    timestamp: number;
    onView: () => void;
  }

  let { pinned, direction, peerName, timestamp, onView }: Props = $props();

  const who = $derived(direction === 'sent' ? t('chat.conversation.you') : peerName);
  const label = $derived(
    pinned ? t('chat.pin.recordPinned', { name: who }) : t('chat.pin.recordUnpinned', { name: who })
  );
</script>

<div class="pin-line">
  {#if pinned}<Pin size={13} />{:else}<PinOff size={13} />{/if}
  <span class="pin-line-text">{label}</span>
  <button class="pin-line-view" onclick={onView}>{t('chat.pin.recordView')}</button>
  <time class="pin-line-time">{formatTime(timestamp)}</time>
</div>

<style>
  .pin-line {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    align-self: center;
    margin: 10px auto;
    padding: 5px 14px;
    font-size: 12px;
    color: var(--text-secondary);
    background: var(--bg-secondary);
    border: 1px solid var(--border-light);
    border-radius: 999px;
  }
  /* Translucency only where color-mix() exists: a color-mix() value containing
     var() computes to `transparent` on engines without it (Chrome < 111). */
  @supports (background: color-mix(in srgb, red 50%, transparent)) {
    .pin-line {
      background: color-mix(in srgb, var(--bg-secondary) 78%, transparent);
    }
  }
  .pin-line :global(svg) {
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .pin-line-text {
    font-weight: 600;
  }
  .pin-line-view {
    padding: 0;
    font-size: 12px;
    font-weight: 600;
    color: var(--accent-hover);
    text-decoration: underline;
    text-underline-offset: 2px;
  }
  .pin-line-view:hover {
    color: var(--accent);
  }
  .pin-line-time {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    font-size: 11px;
  }
</style>
