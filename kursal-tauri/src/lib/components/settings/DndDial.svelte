<script lang="ts">
  import { t } from '$lib/i18n';

  let {
    start,
    end,
    onchange,
    disabled = false,
  }: {
    start: string;
    end: string;
    onchange?: (start: string, end: string) => void;
    disabled?: boolean;
  } = $props();

  const SNAP = 15;
  const MIN_DURATION = 15;
  const MAX_DURATION = 1440 - 15;
  const R = 100;
  const HANDLE_R = 12;

  function parseTime(s: string): number {
    const parts = (s || '0:0').split(':');
    const h = Math.max(0, Math.min(23, parseInt(parts[0], 10) || 0));
    const m = Math.max(0, Math.min(59, parseInt(parts[1], 10) || 0));
    return h * 60 + m;
  }

  function pad(n: number) {
    return n.toString().padStart(2, '0');
  }

  function fmtTime(min: number): string {
    const m = ((min % 1440) + 1440) % 1440;
    return `${pad(Math.floor(m / 60))}:${pad(m % 60)}`;
  }

  function fmtDuration(min: number): string {
    const h = Math.floor(min / 60);
    const m = min % 60;
    if (h === 0) return `${m}m`;
    if (m === 0) return `${h}h`;
    return `${h}h ${m}m`;
  }

  function snap(min: number): number {
    return (((Math.round(min / SNAP) * SNAP) % 1440) + 1440) % 1440;
  }

  function polar(angleDeg: number, r: number, cx = 150, cy = 150) {
    const rad = ((angleDeg - 90) * Math.PI) / 180;
    return { x: cx + Math.cos(rad) * r, y: cy + Math.sin(rad) * r };
  }

  function arcPath(startAng: number, endAng: number, r: number): string {
    const s = polar(startAng, r);
    const e = polar(endAng, r);
    const sweep = (endAng - startAng + 360) % 360 || 360;
    const largeArc = sweep > 180 ? 1 : 0;
    return `M ${s.x} ${s.y} A ${r} ${r} 0 ${largeArc} 1 ${e.x} ${e.y}`;
  }

  const startMin = $derived(parseTime(start));
  const endMin = $derived(parseTime(end));
  const duration = $derived((endMin - startMin + 1440) % 1440 || MIN_DURATION);

  const startAngle = $derived((startMin / 1440) * 360);
  const endAngle = $derived((endMin / 1440) * 360);
  const startPos = $derived(polar(startAngle, R));
  const endPos = $derived(polar(endAngle, R));
  const arcD = $derived(arcPath(startAngle, endAngle, R));

  let svgEl = $state<SVGSVGElement | null>(null);
  let dragMode = $state<'start' | 'end' | 'range' | null>(null);
  let focused = $state<'start' | 'end' | null>(null);
  let dragOffset = 0;

  function minuteAtPointer(e: PointerEvent): number | null {
    if (!svgEl) return null;
    const rect = svgEl.getBoundingClientRect();
    const x = ((e.clientX - rect.left) / rect.width) * 300 - 150;
    const y = ((e.clientY - rect.top) / rect.height) * 300 - 150;
    let angle = (Math.atan2(y, x) * 180) / Math.PI + 90;
    if (angle < 0) angle += 360;
    return (angle / 360) * 1440;
  }

  function distAtPointer(e: PointerEvent): number | null {
    if (!svgEl) return null;
    const rect = svgEl.getBoundingClientRect();
    const x = ((e.clientX - rect.left) / rect.width) * 300 - 150;
    const y = ((e.clientY - rect.top) / rect.height) * 300 - 150;
    return Math.sqrt(x * x + y * y);
  }

  function angularDistMin(a: number, b: number): number {
    const d = Math.abs(a - b) % 1440;
    return Math.min(d, 1440 - d);
  }

  function commit(s: number, e: number) {
    onchange?.(fmtTime(s), fmtTime(e));
  }

  function adjustStart(newStart: number) {
    newStart = snap(newStart);
    const dur = (endMin - newStart + 1440) % 1440;
    if (dur < MIN_DURATION) return;
    if (dur > MAX_DURATION) return;
    commit(newStart, endMin);
  }

  function adjustEnd(newEnd: number) {
    newEnd = snap(newEnd);
    const dur = (newEnd - startMin + 1440) % 1440;
    if (dur < MIN_DURATION) return;
    if (dur > MAX_DURATION) return;
    commit(startMin, newEnd);
  }

  function moveRangeTo(newStart: number) {
    newStart = snap(newStart);
    const dur = duration;
    const newEnd = (newStart + dur) % 1440;
    commit(newStart, newEnd);
  }

  function onPointerDown(e: PointerEvent) {
    if (disabled || !svgEl) return;
    const dist = distAtPointer(e);
    if (dist === null || dist < 70 || dist > 135) return;
    const min = minuteAtPointer(e);
    if (min === null) return;

    const TOL = 45;
    const dStart = angularDistMin(min, startMin);
    const dEnd = angularDistMin(min, endMin);
    const offsetInArc = (min - startMin + 1440) % 1440;
    const onArc = offsetInArc <= duration;

    if (dStart <= TOL && dStart <= dEnd) {
      dragMode = 'start';
    } else if (dEnd <= TOL) {
      dragMode = 'end';
    } else if (onArc) {
      dragMode = 'range';
      dragOffset = offsetInArc;
    } else {
      return;
    }

    try {
      svgEl.setPointerCapture(e.pointerId);
    } catch {}
    e.preventDefault();
    applyDrag(e);
  }

  function applyDrag(e: PointerEvent) {
    if (!dragMode) return;
    const min = minuteAtPointer(e);
    if (min === null) return;
    if (dragMode === 'start') adjustStart(min);
    else if (dragMode === 'end') adjustEnd(min);
    else if (dragMode === 'range') moveRangeTo((min - dragOffset + 1440) % 1440);
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragMode) return;
    applyDrag(e);
  }

  function onPointerUp(e: PointerEvent) {
    if (!dragMode) return;
    dragMode = null;
    try {
      svgEl?.releasePointerCapture(e.pointerId);
    } catch {}
  }

  const hourTicks = Array.from({ length: 24 }, (_, i) => i);
  const hourLabels = [0, 3, 6, 9, 12, 15, 18, 21];
</script>

<div class="dial-root" data-disabled={disabled}>
  <svg
    bind:this={svgEl}
    viewBox="0 0 300 300"
    class="dial"
    role="group"
    aria-label={t('settings.account.dndScheduleAria')}
    onpointerdown={onPointerDown}
    onpointermove={onPointerMove}
    onpointerup={onPointerUp}
    onpointercancel={onPointerUp}
  >
    <circle cx="150" cy="150" r={R} class="track" />

    {#each hourTicks as h}
      {@const major = h % 6 === 0}
      {@const a = h * 15}
      {@const p1 = polar(a, 114)}
      {@const p2 = polar(a, major ? 122 : 118)}
      <line x1={p1.x} y1={p1.y} x2={p2.x} y2={p2.y} class="tick" data-major={major} />
    {/each}

    {#each hourLabels as h}
      {@const p = polar(h * 15, 135)}
      <text x={p.x} y={p.y} class="hour-label">{h}</text>
    {/each}

    <path d={arcD} class="arc" data-dragging={dragMode !== null} />

    <text x="150" y="142" class="center-time">
      {fmtTime(startMin)} → {fmtTime(endMin)}
    </text>
    <text x="150" y="164" class="center-dur">
      {fmtDuration(duration)} muted
    </text>

    <g class="handle-g" data-active={dragMode === 'start' || focused === 'start'}>
      <circle cx={startPos.x} cy={startPos.y} r={HANDLE_R + 10} class="handle-hit" />
      <circle cx={startPos.x} cy={startPos.y} r={HANDLE_R} class="handle" />
      <circle cx={startPos.x} cy={startPos.y} r={HANDLE_R - 5} class="handle-inner" />
      <circle
        cx={startPos.x}
        cy={startPos.y}
        r={HANDLE_R}
        class="handle-focus-target"
        role="slider"
        tabindex={disabled ? -1 : 0}
        aria-label={t('settings.account.dndMuteStartAria')}
        aria-valuemin={0}
        aria-valuemax={1439}
        aria-valuenow={startMin}
        aria-valuetext={fmtTime(startMin)}
        onfocus={() => (focused = 'start')}
        onblur={() => (focused = focused === 'start' ? null : focused)}
      />
    </g>

    <g class="handle-g" data-active={dragMode === 'end' || focused === 'end'}>
      <circle cx={endPos.x} cy={endPos.y} r={HANDLE_R + 10} class="handle-hit" />
      <circle cx={endPos.x} cy={endPos.y} r={HANDLE_R} class="handle" />
      <circle cx={endPos.x} cy={endPos.y} r={HANDLE_R - 5} class="handle-inner" />
      <circle
        cx={endPos.x}
        cy={endPos.y}
        r={HANDLE_R}
        class="handle-focus-target"
        role="slider"
        tabindex={disabled ? -1 : 0}
        aria-label={t('settings.account.dndMuteEndAria')}
        aria-valuemin={0}
        aria-valuemax={1439}
        aria-valuenow={endMin}
        aria-valuetext={fmtTime(endMin)}
        onfocus={() => (focused = 'end')}
        onblur={() => (focused = focused === 'end' ? null : focused)}
      />
    </g>
  </svg>
</div>

<style>
  .dial-root {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 0 2px;
  }
  .dial-root[data-disabled='true'] {
    opacity: 0.5;
    pointer-events: none;
  }
  .dial {
    width: 280px;
    max-width: 100%;
    height: auto;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
  }
  .track {
    fill: none;
    stroke: var(--bg-input);
    stroke-width: 18;
  }
  .tick {
    stroke: var(--text-muted);
    stroke-width: 1;
    opacity: 0.45;
    pointer-events: none;
  }
  .tick[data-major='true'] {
    stroke: var(--text-secondary);
    stroke-width: 1.5;
    opacity: 0.9;
  }
  .hour-label {
    fill: var(--text-muted);
    font-size: var(--text-2xs);
    font-variant-numeric: tabular-nums;
    text-anchor: middle;
    dominant-baseline: central;
    pointer-events: none;
    user-select: none;
  }
  .arc {
    fill: none;
    stroke: var(--accent);
    stroke-width: 18;
    stroke-linecap: round;
    cursor: grab;
    transition: stroke-width 120ms ease;
  }
  .arc[data-dragging='true'] {
    cursor: grabbing;
    stroke-width: 20;
  }
  .center-time {
    fill: var(--text-primary);
    font-size: 18px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    text-anchor: middle;
    dominant-baseline: central;
    pointer-events: none;
  }
  .center-dur {
    fill: var(--text-muted);
    font-size: var(--text-2xs);
    text-anchor: middle;
    dominant-baseline: central;
    pointer-events: none;
  }
  .handle-hit {
    fill: transparent;
    cursor: grab;
  }
  .handle-g[data-active='true'] .handle-hit {
    cursor: grabbing;
  }
  .handle {
    fill: var(--accent);
    stroke: var(--bg-tertiary);
    stroke-width: 3;
    pointer-events: none;
    transition: r 120ms ease;
  }
  .handle-inner {
    fill: #fff;
    pointer-events: none;
    opacity: 0.95;
  }
  .handle-focus-target {
    fill: transparent;
    outline: none;
  }
  .handle-focus-target:focus-visible {
    outline: none;
  }
  .handle-g[data-active='true'] .handle {
    filter: drop-shadow(0 0 0 3px var(--accent-dim));
  }
  .handle-g[data-active='true'] .handle-inner {
    fill: var(--bg-tertiary);
  }
</style>
