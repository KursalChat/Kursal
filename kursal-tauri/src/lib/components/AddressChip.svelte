<script lang="ts">
  import { parseMultiaddr, shortPeerId } from '#lib/utils/multiaddr.js';
  import { t } from '#lib/i18n/index.js';

  let { addr, addresses }: { addr: string; addresses?: string[] } = $props();

  const allAddresses = $derived(addresses ?? [addr]);
  const infos = $derived(allAddresses.map(parseMultiaddr));
  const info = $derived(infos[0] ?? parseMultiaddr(addr));
  const peer = $derived(shortPeerId(info.peerId));
  const primary = $derived(
    info.host ? (info.port ? `${info.host}:${info.port}` : info.host) : (peer ?? addr)
  );
  const showPeerSecondary = $derived(Boolean(info.host && peer));
  const badges = $derived.by(() => {
    const transports = [
      ...new Set(
        infos.map((item) => item.transport).filter((transport): transport is string => !!transport)
      ),
    ];
    const relay = infos.some((item) => item.relay);
    const label = relay
      ? transports.length > 0
        ? t('common.relayBadgeVia', { transport: transports.join(' / ') })
        : t('common.relayBadge')
      : transports.join(' / ');
    return label ? [{ label, relay }] : [];
  });
</script>

<div class="addr" title={allAddresses.join('\n')}>
  {#each badges as badge (badge.label)}
    <span class="badge" class:relay={badge.relay}>{badge.label}</span>
  {/each}
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
