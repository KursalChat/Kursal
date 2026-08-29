<script lang="ts">
  import { CloudUpload, Clock } from 'lucide-svelte';

  type Variant = 'day' | 'time' | 'unread' | 'offline-stored' | 'offline-waiting';

  interface Props {
    variant: Variant;
    label: string;
    ariaLabel?: string;
  }

  let { variant, label, ariaLabel }: Props = $props();
</script>

{#if variant === 'offline-stored' || variant === 'offline-waiting'}
  <div class="offline-section" class:waiting={variant === 'offline-waiting'}>
    {#if variant === 'offline-stored'}
      <CloudUpload size={13} />
    {:else}
      <Clock size={13} />
    {/if}
    <span>{label}</span>
  </div>
{:else if variant === 'unread'}
  <div class="unread-separator" aria-label={ariaLabel}>
    <span>{label}</span>
  </div>
{:else if variant === 'time'}
  <div class="time-separator">
    <span>{label}</span>
  </div>
{:else}
  <div class="day-separator">
    <span>{label}</span>
  </div>
{/if}

<style>
  .day-separator {
    display: flex;
    justify-content: center;
    margin: 18px 0 12px;
    position: sticky;
    top: 6px;
    z-index: 4;
    pointer-events: none;
  }
  .day-separator span {
    font-size: var(--text-2xs);
    font-weight: 700;
    color: var(--text-secondary);
    background: var(--bg-secondary);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
    border: 1px solid var(--border-light);
    border-radius: 999px;
    padding: 4px 12px;
    letter-spacing: 0.02em;
  }
  /* Translucency only where color-mix() exists: a color-mix() value containing
     var() computes to `transparent` on engines without it (Chrome < 111). */
  @supports (background: color-mix(in srgb, red 50%, transparent)) {
    .day-separator span {
      background: color-mix(in srgb, var(--bg-secondary) 86%, transparent);
    }
  }

  .time-separator {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 22px 4px 10px;
    pointer-events: none;
  }
  .time-separator::before,
  .time-separator::after {
    content: '';
    flex: 1;
    height: 1px;
    background: linear-gradient(
      to right,
      transparent,
      var(--border) 30%,
      var(--border) 70%,
      transparent
    );
  }
  .time-separator span {
    font-size: var(--text-2xs);
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-variant-numeric: tabular-nums;
  }

  /* Labels for the two offline tiers at the bottom of the thread: stored in the
     DHT (accent) above still-waiting (muted). Side lines like time-separator. */
  .offline-section {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    margin: 18px 4px 8px;
    font-size: var(--text-2xs);
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--accent-hover);
    pointer-events: none;
  }
  .offline-section.waiting {
    color: var(--text-muted);
  }
  .offline-section::before,
  .offline-section::after {
    content: '';
    flex: 1;
    height: 1px;
    background: linear-gradient(
      to right,
      transparent,
      color-mix(in srgb, currentColor 35%, transparent) 40%,
      color-mix(in srgb, currentColor 35%, transparent) 60%,
      transparent
    );
  }
  .offline-section :global(svg) {
    flex-shrink: 0;
    opacity: 0.9;
  }

  .unread-separator {
    display: flex;
    align-items: center;
    gap: 10px;
    margin: 12px 0 6px;
    animation: sep-in 0.3s ease both;
    color: var(--danger);
  }
  @keyframes sep-in {
    from {
      opacity: 0;
      transform: scaleX(0.7);
    }
    to {
      opacity: 1;
      transform: scaleX(1);
    }
  }
  .unread-separator::before,
  .unread-separator::after {
    content: '';
    flex: 1;
    height: 1px;
    background: currentColor;
    opacity: 0.5;
  }
  .unread-separator span {
    font-size: 10.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
</style>
