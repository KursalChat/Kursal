<script lang="ts">
  import { fly } from 'svelte/transition';
  import { updateDownloadState } from '$lib/state/updateDownload.svelte';
  import { t } from '$lib/i18n';

  const RADIUS = 8;
  const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

  const version = $derived(updateDownloadState.version);
  const downloaded = $derived(updateDownloadState.downloaded);
  const total = $derived(updateDownloadState.total);
  const done = $derived(updateDownloadState.done);
  const message = $derived(updateDownloadState.message);
  const ratio = $derived(message ? 1 : total ? Math.min(1, downloaded / total) : null);
  const label = $derived(version ? t('updateDownload.titleVersion', { version }) : '');

  const INTRO_MS = 2600;
  let intro = $state(false);

  // Says which version it is once, then gets out of the way.
  $effect(() => {
    if (!updateDownloadState.active || !version) {
      intro = false;
      return;
    }
    intro = true;
    const timer = setTimeout(() => (intro = false), INTRO_MS);
    return () => clearTimeout(timer);
  });

  function unitFor(bytes: number): { unit: string; div: number; decimals: number } {
    if (bytes >= 1024 * 1024) return { unit: 'MB', div: 1024 * 1024, decimals: 1 };
    if (bytes >= 1024) return { unit: 'KB', div: 1024, decimals: 1 };
    return { unit: 'B', div: 1, decimals: 0 };
  }

  // Both sides share the larger value's unit, so the pair reads as one number.
  function sizeText(): string {
    const { unit, div, decimals } = unitFor(total ?? downloaded);
    const doneText = (downloaded / div).toFixed(decimals);
    if (!total) return `${doneText} ${unit}`;
    return `${doneText} / ${(total / div).toFixed(decimals)} ${unit}`;
  }

  const text = $derived(
    message ?? (done ? t('updateDownload.installing') : intro ? label : sizeText()),
  );
</script>

{#if updateDownloadState.active}
  <div
    class="update-ring"
    class:expanded={intro || done || !!message}
    role="status"
    aria-label={message ?? (label || undefined)}
    transition:fly={{ y: -8, duration: 160 }}
  >
    <svg class="ring" class:spin={ratio === null && !done} viewBox="0 0 20 20" aria-hidden="true">
      <circle class="track" cx="10" cy="10" r={RADIUS} />
      <circle
        class="arc"
        cx="10"
        cy="10"
        r={RADIUS}
        stroke-dasharray={ratio === null && !done
          ? `${CIRCUMFERENCE * 0.25} ${CIRCUMFERENCE}`
          : CIRCUMFERENCE}
        stroke-dashoffset={ratio === null || done ? 0 : CIRCUMFERENCE * (1 - ratio)}
      />
    </svg>
    <span class="reveal" aria-hidden="true">{text}</span>
  </div>
{/if}

<style>
  .update-ring {
    position: fixed;
    top: calc(12px + var(--safe-top));
    right: calc(12px + var(--safe-right));
    z-index: 9998;
    display: flex;
    align-items: center;
    padding: 5px;
    border-radius: 999px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    box-shadow: 0 6px 18px rgba(0, 0, 0, 0.28);
  }
  .ring {
    width: 18px;
    height: 18px;
    flex-shrink: 0;
    transform: rotate(-90deg);
  }
  .ring circle {
    fill: none;
    stroke-width: 2.5;
    stroke-linecap: round;
  }
  .track {
    stroke: var(--surface-soft);
  }
  .arc {
    stroke: var(--accent);
    transition: stroke-dashoffset 200ms ease;
  }
  .ring.spin {
    animation: update-spin 900ms linear infinite;
  }
  @keyframes update-spin {
    from {
      transform: rotate(-90deg);
    }
    to {
      transform: rotate(270deg);
    }
  }

  .reveal {
    max-width: 0;
    opacity: 0;
    overflow: hidden;
    white-space: nowrap;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
    transition:
      max-width 220ms ease,
      opacity 140ms ease,
      margin 220ms ease;
  }
  .update-ring:hover .reveal,
  .update-ring.expanded .reveal {
    max-width: 220px;
    opacity: 1;
    margin: 0 5px 0 7px;
  }

  @media (prefers-reduced-motion: reduce) {
    .ring.spin {
      animation: none;
    }
    .arc,
    .reveal {
      transition: none;
    }
  }
</style>
