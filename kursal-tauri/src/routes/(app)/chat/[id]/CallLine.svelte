<script lang="ts">
  import { Phone } from 'lucide-svelte';
  import { formatTime } from '$lib/utils/dateFormat.svelte';
  import { callRecordLabel, isMissedCall, type CallRecord } from './chat-utils';

  interface Props {
    rec: CallRecord;
    direction: string;
    timestamp: number;
  }

  let { rec, direction, timestamp }: Props = $props();
</script>

<div class="call-line" class:missed={isMissedCall(rec)}>
  <Phone size={13} />
  <span class="call-line-text">{callRecordLabel(rec, direction)}</span>
  <time class="call-line-time">{formatTime(timestamp)}</time>
</div>

<style>
  .call-line {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    align-self: center;
    margin: 10px auto;
    padding: 5px 14px;
    font-size: var(--text-xs);
    color: var(--text-secondary);
    background: var(--bg-secondary);
    border: 1px solid var(--border-light);
    border-radius: 999px;
  }
  /* Translucency only where color-mix() exists: a color-mix() value containing
     var() computes to `transparent` on engines without it (Chrome < 111). */
  @supports (background: color-mix(in srgb, red 50%, transparent)) {
    .call-line {
      background: color-mix(in srgb, var(--bg-secondary) 78%, transparent);
    }
  }
  .call-line :global(svg) {
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .call-line.missed {
    color: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 35%, var(--border-light));
    background: color-mix(in srgb, var(--danger) 8%, transparent);
  }
  .call-line.missed :global(svg) {
    color: var(--danger);
  }
  .call-line-text {
    font-weight: 600;
  }
  .call-line-time {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    font-size: var(--text-2xs);
  }
</style>
