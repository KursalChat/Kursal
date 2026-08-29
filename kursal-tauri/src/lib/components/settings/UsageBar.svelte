<script lang="ts">
  import { formatBytes } from '$lib/utils/bytes';

  export interface UsageSegment {
    id: string;
    label: string;
    bytes: number;
    pct: number;
    color: string;
  }

  let {
    title,
    total,
    segments,
    emptyLegend,
  }: {
    title: string;
    total: number;
    segments: UsageSegment[];
    emptyLegend?: string;
  } = $props();
</script>

<div class="usage-row">
  <div class="usage-row-head">
    <span class="usage-row-title">{title}</span>
    <span class="usage-row-total mono">{formatBytes(total)}</span>
  </div>
  <div class="bar" class:bar-empty={segments.length === 0}>
    {#each segments as s (s.id)}
      <div
        class="bar-seg"
        style="width: {s.pct}%; background: {s.color};"
        title="{s.label}: {formatBytes(s.bytes)}"
      ></div>
    {/each}
  </div>
  {#if segments.length > 0}
    <div class="legend">
      {#each segments as s (s.id)}
        <div class="legend-item">
          <span class="dot" style="background: {s.color};"></span>
          <span class="legend-label ellipsis">{s.label}</span>
          <span class="mono legend-bytes">{formatBytes(s.bytes)}</span>
        </div>
      {/each}
    </div>
  {:else if emptyLegend}
    <div class="legend muted-empty">{emptyLegend}</div>
  {/if}
</div>

<style>
  .usage-row {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .usage-row-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 12px;
  }
  .usage-row-title {
    font-size: var(--text-xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }
  .usage-row-total {
    font-size: var(--text-sm);
    color: var(--text-primary);
  }
  .bar {
    display: flex;
    width: 100%;
    height: 18px;
    border-radius: var(--radius-md);
    overflow: hidden;
    background: var(--bg-input);
    border: 1px solid var(--border-light);
  }
  .bar-empty {
    opacity: 0.5;
  }
  .bar-seg {
    height: 100%;
    min-width: 2px;
    transition: width var(--transition);
  }
  .bar-seg + .bar-seg {
    border-left: 1px solid rgba(0, 0, 0, 0.18);
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 14px;
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }
  .legend-item {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .legend-label {
    max-width: 160px;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .legend-bytes {
    color: var(--text-muted);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: var(--radius-md);
    flex: 0 0 auto;
  }
  .muted-empty {
    color: var(--text-muted);
    font-size: var(--text-xs);
  }
  .mono {
    font-family: var(--font-mono);
  }
</style>
