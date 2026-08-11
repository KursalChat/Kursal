<script lang="ts" module>
  import type { NodeStats } from '$lib/api/settings';

  const SPARK_LEN = 30;

  type SeriesKey = 'cpu' | 'mem' | 'in' | 'out';

  // Module scope so sparkline history survives navigating away and back;
  // component state would reset and leave the charts blank for two polls.
  let nodeStats = $state<NodeStats | null>(null);
  let series = $state<Record<SeriesKey, { hist: number[]; ceil: number }>>({
    cpu: { hist: [], ceil: 0 },
    mem: { hist: [], ceil: 0 },
    in: { hist: [], ceil: 0 },
    out: { hist: [], ceil: 0 },
  });

  // Round up to 1/2/5 x 10^n so the y-domain only moves at meaningful thresholds.
  function niceCeil(v: number): number {
    if (!(v > 0)) return 1;
    const mag = Math.pow(10, Math.floor(Math.log10(v)));
    const n = v / mag;
    return (n <= 1 ? 1 : n <= 2 ? 2 : n <= 5 ? 5 : 10) * mag;
  }

  function pushSample(key: SeriesKey, raw: number) {
    const s = series[key];
    const v = Number.isFinite(raw) ? Math.max(raw, 0) : 0;
    const hist = s.hist;
    hist.push(v);
    if (hist.length > SPARK_LEN) hist.splice(0, hist.length - SPARK_LEN);
    let max = 0;
    for (const n of hist) if (n > max) max = n;
    // Grow onto a new peak at once, but shrink only once the window falls to a
    // quarter of the domain, and then only to half-height. Rescaling to the window
    // max on every scroll-off drew a rising line for what was really a falling value.
    const target = niceCeil(max);
    if (target > s.ceil) s.ceil = target;
    else if (max <= s.ceil / 4) s.ceil = niceCeil(max * 2);
  }

  // Fixed 0..ceil domain: height is always proportional to the real value.
  function sparkPoints(data: number[], ceil: number): string {
    if (data.length === 0 || ceil <= 0) return '';
    // One sample draws flat so a tile is never blank while history fills.
    const pts = data.length === 1 ? [data[0], data[0]] : data;
    const w = 100;
    const h = 24;
    const pad = 2;
    return pts
      .map((v, i) => {
        const x = (i / (pts.length - 1)) * w;
        const y = h - pad - Math.min(Math.max(v / ceil, 0), 1) * (h - pad * 2);
        return `${x.toFixed(1)},${y.toFixed(1)}`;
      })
      .join(' ');
  }
</script>

<script lang="ts">
  import { goto } from '$app/navigation';
  import { slide } from 'svelte/transition';
  import { PhoneMissed, Pencil, Check, Sparkles, ChevronDown, UserPlus } from 'lucide-svelte';
  import Avatar from '$lib/components/Avatar.svelte';
  import StatusDot from '$lib/components/StatusDot.svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { messagesState } from '$lib/state/messages.svelte';
  import { profileState } from '$lib/state/profile.svelte';
  import { networkState } from '$lib/state/network.svelte';
  import { appFocusState } from '$lib/state/appFocus.svelte';
  import { draftsState } from '$lib/state/drafts.svelte';
  import { groupLabel, latestEntry, olderEntries } from '$lib/changelog';
  import { getNodeStats } from '$lib/api/settings';
  import { formatFileSize } from './chat/[id]/chat-utils';
  import type { ContactResponse } from '$lib/types';
  import { t } from '$lib/i18n';

  $effect(() => {
    if (!appFocusState.focused) return;
    let cancelled = false;
    const poll = async () => {
      try {
        const s = await getNodeStats();
        if (cancelled) return;
        nodeStats = s;
        if (s.memBytes > 0) {
          pushSample('cpu', s.cpuPercent);
          pushSample('mem', s.memBytes);
        }
        pushSample('in', s.rateIn);
        pushSample('out', s.rateOut);
      } catch {
        if (!cancelled) nodeStats = null;
      }
    };
    void poll();
    const timer = setInterval(poll, 2000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  });

  const processStatsAvailable = $derived(!!nodeStats && nodeStats.memBytes > 0);

  const greetingKey = (() => {
    const h = new Date().getHours();
    if (h < 5) return 'home.greetingNight';
    if (h < 12) return 'home.greetingMorning';
    if (h < 18) return 'home.greetingAfternoon';
    return 'home.greetingEvening';
  })();

  interface AttentionRow {
    contact: ContactResponse;
    unread: number;
    unreadCapped: boolean;
    missedCall: boolean;
    hasDraft: boolean;
    ts: number;
  }

  const attention = $derived.by(() => {
    const rows: AttentionRow[] = [];
    for (const contact of contactsState.contacts) {
      const unread = messagesState.unreadFor(contact.userId);
      const msgs = messagesState.forContact(contact.userId);
      const last = msgs.length ? msgs[msgs.length - 1] : null;
      const missedCall =
        !!last?.callDetails &&
        last.direction === 'received' &&
        last.callDetails.outcome === 'missed';
      const hasDraft = !!draftsState.get(contact.userId);
      if (unread > 0 || missedCall || hasDraft) {
        rows.push({
          contact,
          unread,
          unreadCapped: messagesState.unreadCappedFor(contact.userId),
          missedCall,
          hasDraft,
          ts: last?.timestamp ?? contact.createdAt * 1000,
        });
      }
    }
    rows.sort((a, b) => (b.unread > 0 ? 1 : 0) - (a.unread > 0 ? 1 : 0) || b.ts - a.ts);
    return rows.slice(0, 8);
  });

  const whatsNew = latestEntry();
  const history = olderEntries();

  // Auto-expanded until the user collapses it once for this version.
  let whatsNewOpen = $state(
    typeof localStorage !== 'undefined' &&
      localStorage.getItem('kursal_whatsnew_seen') !== whatsNew?.version
  );
  let historyOpen = $state(false);

  function toggleWhatsNew() {
    whatsNewOpen = !whatsNewOpen;
    if (!whatsNewOpen && whatsNew) {
      localStorage.setItem('kursal_whatsnew_seen', whatsNew.version);
      historyOpen = false;
    }
  }
</script>

<div class="home" data-tauri-drag-region>
  <div class="home-inner">
    <header class="hero">
      <img class="mascot" src="/winston.webp" alt={t('home.mascotAlt')} width="92" height="92" />
      <div class="hero-text">
        <h2>{t(greetingKey, { name: profileState.displayName })}</h2>
        <p class="status-line">
          <span class="net-dot" class:off={networkState.initialized && !networkState.online}></span>
          {networkState.online ? t('home.statusOnline') : t('home.statusOffline')}
          <span class="sep">·</span>
          {t('home.statusPeers', { n: networkState.peerCount })}
          <span class="sep">·</span>
          {t('home.statusContacts', { n: contactsState.contacts.length })}
        </p>
      </div>
    </header>

    {#if nodeStats}
      {#snippet sparkline(key: SeriesKey, ceilLabel: string)}
        {@const s = series[key]}
        {@const pts = sparkPoints(s.hist, s.ceil)}
        <svg class="spark" viewBox="0 0 100 24" preserveAspectRatio="none" aria-hidden="true">
          <line class="spark-base" x1="0" y1="22" x2="100" y2="22" />
          {#if pts}
            <polyline points={pts} />
          {/if}
        </svg>
        <span class="tile-scale">{t('home.tileScale', { max: ceilLabel })}</span>
      {/snippet}
      <section
        class="tile-grid"
        class:net-only={!processStatsAvailable}
        aria-label={t('home.statusHeading')}
      >
        {#if processStatsAvailable}
          <div class="tile">
            <span class="tile-label">{t('home.tileCpu')}</span>
            <span class="tile-value">{nodeStats.cpuPercent.toFixed(1)}%</span>
            {@render sparkline('cpu', `${series.cpu.ceil}%`)}
          </div>
          <div class="tile">
            <span class="tile-label">{t('home.tileMem')}</span>
            <span class="tile-value">{formatFileSize(nodeStats.memBytes)}</span>
            {@render sparkline('mem', formatFileSize(series.mem.ceil))}
          </div>
        {/if}
        <div class="tile">
          <span class="tile-label">{t('home.tileDown')}</span>
          <span class="tile-value">{formatFileSize(Math.round(nodeStats.rateIn)) || '0 B'}/s</span>
          {@render sparkline('in', `${formatFileSize(series.in.ceil)}/s`)}
        </div>
        <div class="tile">
          <span class="tile-label">{t('home.tileUp')}</span>
          <span class="tile-value">{formatFileSize(Math.round(nodeStats.rateOut)) || '0 B'}/s</span>
          {@render sparkline('out', `${formatFileSize(series.out.ceil)}/s`)}
        </div>
      </section>
    {/if}

    <section class="attention" aria-label={t('home.attentionHeading')}>
      <h3>{t('home.attentionHeading')}</h3>
      {#if contactsState.contacts.length === 0}
        <button class="first-contact-cta" onclick={() => goto('/add-contact')}>
          <UserPlus size={15} />
          {t('layout.addFirstContact')}
        </button>
      {:else if attention.length === 0}
        <div class="caught-up">
          <span class="caught-up-icon"><Check size={15} /></span>
          {t('home.allCaughtUp')}
        </div>
      {:else}
        <div class="attention-list">
          {#each attention as row (row.contact.userId)}
            <button class="attention-row" onclick={() => goto('/chat/' + row.contact.userId)}>
              <div class="attention-avatar">
                <Avatar name={row.contact.displayName} src={row.contact.avatarBase64} size={36} />
                <StatusDot
                  status={contactsState.connectionStatus[row.contact.userId] ?? 'disconnected'}
                />
              </div>
              <span class="attention-name">{row.contact.displayName}</span>
              <span class="chips">
                {#if row.missedCall}
                  <span class="chip missed">
                    <PhoneMissed size={11} />
                    {t('home.missedCall')}
                  </span>
                {/if}
                {#if row.hasDraft}
                  <span class="chip draft">
                    <Pencil size={11} />
                    {t('home.draft')}
                  </span>
                {/if}
                {#if row.unread > 0}
                  <span class="badge"
                    >{row.unread > 99 || row.unreadCapped ? '99+' : row.unread}</span
                  >
                {/if}
              </span>
            </button>
          {/each}
        </div>
      {/if}
    </section>

    {#if whatsNew}
      <section class="whats-new" aria-label={t('home.whatsNew')}>
        <button class="whats-new-header" onclick={toggleWhatsNew} aria-expanded={whatsNewOpen}>
          <span class="whats-new-title">
            <Sparkles size={14} />
            {t('home.whatsNew')}
          </span>
          <span class="version-tag">v{whatsNew.version}</span>
          <span class="chevron" class:up={whatsNewOpen}>
            <ChevronDown size={15} />
          </span>
        </button>
        {#if whatsNewOpen}
          <div class="whats-new-body" transition:slide={{ duration: 220 }}>
            <div class="groups">
              {#each whatsNew.groups as group (group.kind)}
                <div class="group" data-kind={group.kind}>
                  <span class="group-title">{groupLabel(group.kind)}</span>
                  <ul>
                    {#each group.items as item (item)}
                      <li>{item}</li>
                    {/each}
                  </ul>
                </div>
              {/each}
            </div>
            {#if history.length > 0}
              <button
                class="history-toggle"
                onclick={() => (historyOpen = !historyOpen)}
                aria-expanded={historyOpen}
              >
                <span class="chevron" class:up={historyOpen}>
                  <ChevronDown size={13} />
                </span>
                {t('home.olderVersions')}
              </button>
              {#if historyOpen}
                <div class="history" transition:slide={{ duration: 200 }}>
                  {#each history as entry (entry.version)}
                    <div class="history-entry">
                      <span class="version-tag">v{entry.version}</span>
                      <div class="groups">
                        {#each entry.groups as group (group.kind)}
                          <div class="group" data-kind={group.kind}>
                            <span class="group-title">{groupLabel(group.kind)}</span>
                            <ul>
                              {#each group.items as item (item)}
                                <li>{item}</li>
                              {/each}
                            </ul>
                          </div>
                        {/each}
                      </div>
                    </div>
                  {/each}
                </div>
              {/if}
            {/if}
          </div>
        {/if}
      </section>
    {/if}
  </div>
</div>

<style>
  .home {
    flex: 1;
    overflow-y: auto;
    padding: 24px max(24px, var(--safe-right)) calc(24px + var(--safe-bottom))
      max(24px, var(--safe-left));
    display: flex;
    flex-direction: column;
  }

  @media (min-width: 769px) {
    .home {
      padding-top: calc(24px + var(--safe-top));
    }
  }
  .home-inner {
    width: 100%;
    max-width: 520px;
    margin: auto;
    display: flex;
    flex-direction: column;
    gap: 26px;
  }

  .hero {
    display: flex;
    align-items: center;
    gap: 18px;
    animation: rise 0.4s cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  .mascot {
    flex-shrink: 0;
    position: relative;
    filter: drop-shadow(0 6px 18px color-mix(in srgb, var(--accent) 30%, transparent));
  }
  .hero-text {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .hero-text h2 {
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.015em;
    color: var(--text-primary);
    line-height: 1.2;
  }
  .status-line {
    display: flex;
    align-items: center;
    gap: 7px;
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .net-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--success);
    box-shadow: 0 0 6px color-mix(in srgb, var(--success) 60%, transparent);
    flex-shrink: 0;
  }
  .net-dot.off {
    background: var(--warning);
    box-shadow: none;
  }
  .sep {
    color: var(--border-light);
  }

  section {
    animation: rise 0.4s cubic-bezier(0.22, 1, 0.36, 1) both;
  }
  .attention {
    animation-delay: 60ms;
  }
  .whats-new {
    animation-delay: 120ms;
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(10px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .hero,
    section {
      animation: none;
    }
  }

  .tile-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
  }
  .tile-grid.net-only {
    grid-template-columns: repeat(2, 1fr);
  }
  .tile {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: 10px 12px 8px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    min-width: 0;
  }
  .tile-label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--text-muted);
    white-space: nowrap;
  }
  .tile-value {
    font-size: 14.5px;
    font-weight: 600;
    color: var(--text-primary);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .spark {
    width: 100%;
    height: 24px;
    margin-top: 2px;
    overflow: visible;
  }
  .spark polyline {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2;
    stroke-linejoin: round;
    stroke-linecap: round;
    vector-effect: non-scaling-stroke;
  }
  .spark-base {
    stroke: var(--border);
    stroke-width: 1;
    vector-effect: non-scaling-stroke;
  }
  .tile-scale {
    font-size: 9px;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  h3 {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
    margin-bottom: 8px;
  }

  .first-contact-cta {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    padding: 10px 18px;
    border-radius: var(--radius-md);
    background: var(--accent);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    transition: background var(--transition);
  }
  .first-contact-cta:hover {
    background: var(--accent-hover);
  }

  .caught-up {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 14px;
    border: 1px dashed var(--border);
    border-radius: var(--radius-md);
    color: var(--text-muted);
    font-size: 13px;
  }
  .caught-up-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    background: var(--accent-dim);
    color: var(--accent);
    flex-shrink: 0;
  }

  .attention-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .attention-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    text-align: left;
    transition: background var(--transition);
  }
  .attention-row:hover {
    background: var(--bg-hover);
  }
  .attention-avatar {
    position: relative;
    flex-shrink: 0;
  }
  .attention-avatar :global(.status-dot) {
    position: absolute;
    bottom: -1px;
    right: -1px;
    border: 2px solid var(--bg-secondary);
  }
  .attention-name {
    flex: 1;
    min-width: 0;
    font-size: 13.5px;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chips {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 600;
  }
  .chip.missed {
    background: var(--danger-dim);
    color: var(--danger);
  }
  .chip.draft {
    background: var(--bg-tertiary);
    color: var(--text-muted);
  }

  .whats-new {
    border: 1px solid var(--border);
    background: var(--bg-tertiary);
    border-radius: var(--radius-md);
    padding: 6px 8px;
  }
  .whats-new-body {
    padding-bottom: 6px;
  }
  .whats-new-header {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px;
    border-radius: var(--radius-sm);
    text-align: left;
    transition: background var(--transition);
  }
  .whats-new-header:hover {
    background: var(--bg-hover);
  }
  .chevron {
    margin-left: auto;
    display: inline-flex;
    color: var(--text-muted);
    transition: transform 150ms ease;
  }
  .chevron.up {
    transform: rotate(180deg);
  }
  .whats-new-title {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    font-weight: 700;
    color: var(--accent);
  }
  .version-tag {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    background: var(--bg-input);
    border: 1px solid var(--border-light);
    border-radius: var(--radius-sm);
    padding: 1px 6px;
  }
  .groups {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 8px;
  }
  .group-title {
    display: block;
    padding-left: 6px;
    font-size: 10.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }
  .group[data-kind='features'] .group-title {
    color: var(--accent);
  }
  .whats-new ul {
    margin: 4px 0 0;
    padding-left: 24px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .whats-new li {
    font-size: 12.5px;
    color: var(--text-secondary);
    line-height: 1.45;
  }
  .history-toggle {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin: 10px 0 0 6px;
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-muted);
    transition: color var(--transition);
  }
  .history-toggle:hover {
    color: var(--text-secondary);
  }
  .history-toggle .chevron {
    margin-left: 0;
  }
  .history {
    display: flex;
    flex-direction: column;
    gap: 10px;
    margin-top: 10px;
    padding: 10px 6px 0;
    border-top: 1px solid var(--border-light);
    max-height: 220px;
    overflow-y: auto;
  }
  .history-entry .groups {
    margin-top: 6px;
    gap: 8px;
  }

  .badge {
    flex-shrink: 0;
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: 999px;
    background: var(--accent);
    color: #fff;
    font-size: 11px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  @media (max-width: 480px) {
    .hero {
      flex-direction: column;
      align-items: flex-start;
      gap: 12px;
    }
    .mascot {
      width: 76px;
      height: 76px;
    }
    .tile-grid {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
