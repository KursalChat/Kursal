<script lang="ts">
  import { parseMultiaddr, shortPeerId } from '$lib/utils/multiaddr';
  import { t } from '$lib/i18n';

  let { addr }: { addr: string } = $props();

  const info = $derived(parseMultiaddr(addr));
  const peer = $derived(shortPeerId(info.peerId));
  const primary = $derived(
    info.host ? (info.port ? `${info.host}:${info.port}` : info.host) : (peer ?? addr)
  );
  const showPeerSecondary = $derived(Boolean(info.host && peer));
  const badge = $derived.by(() => {
    if (!info.relay) return info.transport;
    if (!info.transport) return t('common.relayBadge');
    return t('common.relayBadgeVia', { transport: info.transport });
  });
</script>

<div class="addr" title={addr}>
  {#if badge}
    <span class="badge" class:relay={info.relay}>{badge}</span>
  {/if}
  <span class="primary">{primary}</span>
  {#if showPeerSecondary}
    <span class="secondary">{peer}</span>
  {/if}
</div>

<style>
  .addr {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .badge {
    flex-shrink: 0;
    font-size: var(--text-2xs);
    font-weight: 600;
    color: var(--text-secondary);
    background: var(--bg-subtle, rgba(127, 127, 127, 0.12));
    padding: 2px 7px;
    border-radius: var(--radius-sm, 6px);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }
  .badge.relay {
    color: var(--info);
    background: color-mix(in srgb, var(--info) 12%, transparent);
  }
  .primary {
    flex-shrink: 0;
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 12.5px;
    color: var(--text-primary);
  }
  .secondary {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--text-muted);
  }
</style>
