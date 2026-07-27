<script lang="ts">
  import { RefreshCw } from 'lucide-svelte';
  import { fade } from 'svelte/transition';
  import { offlineSyncState } from '$lib/state/offlineSync.svelte';
  import { t } from '$lib/i18n';

  interface Props {
    // Icon-only, for tight spots like the mobile chat header. The label moves
    // into the tooltip rather than being dropped.
    compact?: boolean;
  }

  let { compact = false }: Props = $props();

  // The indicator only shows while a poll runs, then lingers a short "just now"
  // confirmation before fading out entirely - no permanent between-polls text.
  const LINGER_MS = 30_000;

  let recentlyCompleted = $state(false);
  $effect(() => {
    const at = offlineSyncState.lastCompletedAt;
    if (at === null) return;
    const elapsed = Date.now() - at;
    if (elapsed >= LINGER_MS) {
      recentlyCompleted = false;
      return;
    }
    recentlyCompleted = true;
    const timer = setTimeout(() => (recentlyCompleted = false), LINGER_MS - elapsed);
    return () => clearTimeout(timer);
  });
</script>

{#if offlineSyncState.active}
  <div
    class="offline-sync active"
    class:compact
    title={compact ? t('offlineSync.checking') : t('offlineSync.checkingTitle')}
    in:fade={{ duration: 150 }}
  >
    <RefreshCw size={11} />
    {#if !compact}<span>{t('offlineSync.checking')}</span>{/if}
  </div>
{:else if recentlyCompleted && !compact}
  <div
    class="offline-sync"
    title={t('offlineSync.idleTitle')}
    in:fade={{ duration: 150 }}
    out:fade={{ duration: 500 }}
  >
    <RefreshCw size={11} />
    <span>{t('offlineSync.justNow')}</span>
  </div>
{/if}

<style>
  .offline-sync {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 10.5px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .offline-sync :global(svg) {
    flex-shrink: 0;
    opacity: 0.75;
  }
  .offline-sync.active {
    color: var(--text-secondary);
  }
  .offline-sync.active :global(svg) {
    opacity: 1;
    animation: offline-sync-spin 1.4s linear infinite;
  }
  @keyframes offline-sync-spin {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .offline-sync.active :global(svg) {
      animation: none;
    }
  }
</style>
