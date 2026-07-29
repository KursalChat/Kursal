<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { page } from '$app/state';
  import { goto, beforeNavigate } from '$app/navigation';
  import { fade } from 'svelte/transition';
  import { User, Palette, ShieldCheck, Phone, Wifi, HardDrive, Zap, Search } from 'lucide-svelte';
  import { t, tEn } from '$lib/i18n';
  import { confirmDialog } from '$lib/state/confirm.svelte';
  import { settingsDirty } from '$lib/state/settingsDirty.svelte';
  import AccountSection from '$lib/components/settings/AccountSection.svelte';
  import AppearanceSection from '$lib/components/settings/AppearanceSection.svelte';
  import PrivacySection from '$lib/components/settings/PrivacySection.svelte';
  import CallsSection from '$lib/components/settings/CallsSection.svelte';
  import NetworkSection from '$lib/components/settings/NetworkSection.svelte';
  import StorageSection from '$lib/components/settings/StorageSection.svelte';
  import AdvancedSection from '$lib/components/settings/AdvancedSection.svelte';

  type Category =
    'account' | 'appearance' | 'privacy' | 'calls' | 'network' | 'storage' | 'advanced';

  let activeCategory = $state<Category>('account');
  let bodyEl = $state<HTMLElement | null>(null);

  const CATEGORIES: Category[] = [
    'account',
    'appearance',
    'privacy',
    'calls',
    'network',
    'storage',
    'advanced',
  ];

  onMount(() => {
    settingsDirty.value = false;
  });

  $effect(() => {
    const cat = page.url.searchParams.get('cat');
    if (cat && CATEGORIES.includes(cat as Category)) {
      activeCategory = cat as Category;
    }
  });

  function confirmDiscard(): Promise<boolean> {
    return confirmDialog({
      title: t('settings.unsaved.title'),
      message: t('settings.unsaved.message'),
      confirmLabel: t('settings.unsaved.discard'),
      cancelLabel: t('settings.unsaved.keepEditing'),
      tone: 'warning',
    });
  }

  async function switchCategory(id: Category): Promise<boolean> {
    if (id === activeCategory) return true;
    if (settingsDirty.value && !(await confirmDiscard())) return false;
    settingsDirty.value = false;
    activeCategory = id;
    return true;
  }

  // Guard route changes away from settings (clicking a contact, etc.) so
  // unsaved edits aren't silently dropped. switchCategory covers same-page
  // category switches; this covers actual navigations.
  let bypassGuard = false;
  beforeNavigate((nav) => {
    if (bypassGuard) {
      bypassGuard = false;
      return;
    }
    if (!settingsDirty.value || nav.type === 'leave') return;
    nav.cancel();
    void (async () => {
      if (!(await confirmDiscard())) return;
      settingsDirty.value = false;
      bypassGuard = true;
      if (nav.to) void goto(nav.to.url);
    })();
  });

  $effect(() => {
    activeCategory;
    bodyEl?.scrollTo({ top: 0, behavior: 'instant' });
  });

  const categories = $derived([
    { id: 'account' as const, label: t('settings.categories.account'), icon: User },
    { id: 'appearance' as const, label: t('settings.categories.appearance'), icon: Palette },
    { id: 'privacy' as const, label: t('settings.categories.privacy'), icon: ShieldCheck },
    { id: 'calls' as const, label: t('settings.categories.calls'), icon: Phone },
    { id: 'network' as const, label: t('settings.categories.network'), icon: Wifi },
    { id: 'storage' as const, label: t('settings.categories.storage'), icon: HardDrive },
    { id: 'advanced' as const, label: t('settings.categories.advanced'), icon: Zap },
  ]);

  let settingsQuery = $state('');
  const SEARCH_MIN = 2;

  // Searchable individual settings. `titleKey` is both the result label and the
  // text we match against the rendered .card-title / .row .title to scroll to.
  // `keywords` add synonyms the visible title doesn't contain.
  interface SettingEntry {
    category: Category;
    titleKey: string;
    keywords?: string[];
  }
  const SETTINGS_INDEX: SettingEntry[] = [
    {
      category: 'account',
      titleKey: 'settings.account.profileCard',
      keywords: ['name', 'display name', 'avatar', 'bio'],
    },
    {
      category: 'account',
      titleKey: 'settings.account.backupCard',
      keywords: ['export', 'import', 'long term code', 'ltc', 'recovery'],
    },
    {
      category: 'account',
      titleKey: 'settings.account.notificationsCard',
      keywords: ['preview', 'do not disturb', 'dnd', 'sound'],
    },
    {
      category: 'appearance',
      titleKey: 'settings.appearance.colorThemeCard',
      keywords: ['palette', 'accent'],
    },
    {
      category: 'appearance',
      titleKey: 'settings.appearance.themeRow',
      keywords: ['dark', 'light', 'mode'],
    },
    {
      category: 'appearance',
      titleKey: 'settings.appearance.textSizeRow',
      keywords: ['font', 'zoom', 'scale'],
    },
    {
      category: 'appearance',
      titleKey: 'settings.appearance.languageCard',
      keywords: ['locale', 'translation', 'translate', 'weblate', 'contribute'],
    },
    {
      category: 'appearance',
      titleKey: 'settings.appearance.messageLayoutCard',
      keywords: ['bubble', 'flat', 'compact'],
    },
    {
      category: 'advanced',
      titleKey: 'settings.advanced.autoStartRow',
      keywords: ['startup', 'launch', 'boot', 'autostart'],
    },
    { category: 'privacy', titleKey: 'settings.privacy.typingIndicatorsRow', keywords: ['typing'] },
    {
      category: 'calls',
      titleKey: 'settings.calls.qualityRow',
      keywords: ['call', 'quality', 'sample rate', 'khz', 'audio'],
    },
    {
      category: 'privacy',
      titleKey: 'settings.privacy.identityCard',
      keywords: ['rotation', 'peer id', 'rotate'],
    },
    {
      category: 'privacy',
      titleKey: 'settings.privacy.appLockCard',
      keywords: ['biometric', 'faceid', 'face id', 'touch id', 'passcode', 'lock'],
    },
    {
      category: 'privacy',
      titleKey: 'settings.privacy.blockedCard',
      keywords: ['block', 'contacts'],
    },
    {
      category: 'privacy',
      titleKey: 'settings.privacy.clearHistoryCard',
      keywords: ['delete messages', 'wipe', 'history'],
    },
    {
      category: 'privacy',
      titleKey: 'settings.privacy.dangerCard',
      keywords: ['reset', 'delete account', 'erase'],
    },
    {
      category: 'network',
      titleKey: 'settings.network.discoveryCard',
      keywords: ['nearby', 'mdns', 'local'],
    },
    {
      category: 'network',
      titleKey: 'settings.network.relayCard',
      keywords: ['run as relay', 'server'],
    },
    {
      category: 'network',
      titleKey: 'settings.network.listeningPortRow',
      keywords: ['port', 'tcp', 'quic'],
    },
    {
      category: 'network',
      titleKey: 'settings.network.maxConnectionsRow',
      keywords: ['connections', 'peers'],
    },
    { category: 'storage', titleKey: 'settings.storage.autoAcceptCard', keywords: ['files'] },
    {
      category: 'storage',
      titleKey: 'settings.storage.autoDownloadCard',
      keywords: ['media', 'images'],
    },
    {
      category: 'storage',
      titleKey: 'settings.storage.diskUsageCard',
      keywords: ['disk', 'usage', 'space'],
    },
    {
      category: 'storage',
      titleKey: 'settings.storage.sharedFilesCard',
      keywords: ['cache', 'logs'],
    },
    {
      category: 'advanced',
      titleKey: 'settings.advanced.localApiCard',
      keywords: ['api', 'token', 'host', 'port'],
    },
    {
      category: 'advanced',
      titleKey: 'settings.advanced.backgroundCard',
      keywords: ['background', 'tray', 'keep running', 'menubar', 'close'],
    },
    {
      category: 'advanced',
      titleKey: 'settings.advanced.updatesCard',
      keywords: ['channel', 'beta', 'auto updater'],
    },
    {
      category: 'advanced',
      titleKey: 'settings.advanced.aboutCard',
      keywords: ['version', 'license', 'source', 'docs'],
    },
    {
      category: 'advanced',
      titleKey: 'settings.advanced.benchmarksCard',
      keywords: ['benchmark', 'performance'],
    },
  ];

  type CategoryMeta = (typeof categories)[number];
  const categoryById = $derived(
    Object.fromEntries(categories.map((c) => [c.id, c] as const)) as Record<Category, CategoryMeta>
  );

  // Match against a haystack that always carries the English label too, so an
  // English query still hits when the UI runs in another locale (label would
  // otherwise be translated and never contain the typed word).
  const settingResults = $derived.by(() => {
    const q = settingsQuery.trim().toLowerCase();
    if (q.length < SEARCH_MIN) return [];
    return SETTINGS_INDEX.map((e) => ({ ...e, label: t(e.titleKey) })).filter((e) => {
      const haystack = [t(e.titleKey), tEn(e.titleKey), ...(e.keywords ?? [])]
        .join(' ')
        .toLowerCase();
      return haystack.includes(q);
    });
  });

  const searching = $derived(settingsQuery.trim().length >= SEARCH_MIN);

  function flash(el: Element) {
    el.classList.remove('setting-flash');
    // Force reflow so re-adding the class restarts the CSS animation.
    void (el as HTMLElement).offsetWidth;
    el.classList.add('setting-flash');
    setTimeout(() => el.classList.remove('setting-flash'), 1300);
  }

  async function goToSetting(entry: SettingEntry) {
    if (!(await switchCategory(entry.category))) return;
    settingsQuery = '';
    await tick();
    const target = (t(entry.titleKey) ?? '').trim().toLowerCase();
    const nodes = bodyEl?.querySelectorAll('.card-title, .row .title') ?? [];
    let hit: Element | null = null;
    for (const n of nodes) {
      if ((n.textContent ?? '').trim().toLowerCase() === target) {
        hit = n;
        break;
      }
    }
    const block = hit?.closest('.card-wrap') ?? hit?.closest('.row') ?? hit;
    if (block) {
      block.scrollIntoView({ behavior: 'smooth', block: 'start' });
      flash(block);
    }
  }
</script>

<div class="settings">
  <header class="settings-header" data-tauri-drag-region>
    <h2><span class="prompt">~/</span>{t('settings.heading')}</h2>
  </header>

  <div class="settings-layout">
    <nav class="sidenav" aria-label={t('settings.categoriesAriaLabel')}>
      <div class="settings-search">
        <Search size={14} />
        <input
          type="text"
          placeholder={t('settings.searchPlaceholder')}
          bind:value={settingsQuery}
          spellcheck="false"
          autocorrect="off"
          autocapitalize="off"
        />
      </div>
      <div class="nav-items">
        {#each categories as cat (cat.id)}
          <button
            class="nav-item"
            data-active={!searching && activeCategory === cat.id}
            onclick={() => switchCategory(cat.id)}
          >
            <cat.icon size={15} />
            <span>{cat.label}</span>
          </button>
        {/each}
      </div>
    </nav>

    <section class="settings-body" bind:this={bodyEl}>
      <div class="settings-content">
        {#if searching}
          <div class="search-results" in:fade={{ duration: 140 }}>
            {#if settingResults.length === 0}
              <p class="no-results">
                {t('settings.noResults', { query: settingsQuery.trim() })}
              </p>
            {:else}
              {#each settingResults as r (r.titleKey)}
                {@const cm = categoryById[r.category]}
                <button class="result-item" onclick={() => goToSetting(r)}>
                  <span class="result-icon"><cm.icon size={15} /></span>
                  <span class="result-text">
                    <span class="result-label">{r.label}</span>
                    <span class="result-category">{cm.label}</span>
                  </span>
                </button>
              {/each}
            {/if}
          </div>
        {:else}
          {#key activeCategory}
            <div class="section-fade" in:fade={{ duration: 140 }}>
              {#if activeCategory === 'account'}
                <AccountSection />
              {:else if activeCategory === 'appearance'}
                <AppearanceSection />
              {:else if activeCategory === 'privacy'}
                <PrivacySection />
              {:else if activeCategory === 'calls'}
                <CallsSection />
              {:else if activeCategory === 'network'}
                <NetworkSection />
              {:else if activeCategory === 'storage'}
                <StorageSection />
              {:else if activeCategory === 'advanced'}
                <AdvancedSection />
              {/if}
            </div>
          {/key}
        {/if}
      </div>
    </section>
  </div>
</div>

<style>
  .settings {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .settings-header {
    height: var(--header-height);
    padding: 0 20px;
    display: flex;
    align-items: center;
    background: var(--panel);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    box-shadow: inset 0 -1px 0 var(--panel-border);
    flex-shrink: 0;
  }

  .settings-header h2 {
    margin: 0;
    font-size: var(--text-lg);
    font-weight: 700;
  }
  .prompt {
    color: var(--accent);
    font-weight: 600;
    font-family: var(--font-mono);
  }

  .settings-layout {
    flex: 1;
    display: flex;
    min-height: 0;
    overflow: hidden;
  }

  .sidenav {
    flex-shrink: 0;
    width: 220px;
    padding: 16px 10px;
    background: var(--panel);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    box-shadow: inset -1px 0 0 var(--panel-border);
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow-y: auto;
  }

  .nav-items {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex-shrink: 0;
  }

  .settings-search {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
    margin-bottom: 6px;
    padding: 7px 9px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text-muted);
  }
  .settings-search:focus-within {
    border-color: var(--accent-selected);
  }
  .settings-search input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 13px;
  }
  .settings-search input::placeholder {
    color: var(--text-muted);
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-radius: var(--radius-md);
    font-size: 13px;
    font-weight: 500;
    color: var(--text-secondary);
    text-align: left;
    transition:
      background var(--transition),
      color var(--transition);
    min-width: 0;
  }
  .nav-item :global(svg) {
    flex-shrink: 0;
  }
  .nav-item span {
    min-width: 0;
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .nav-item:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .nav-item[data-active='true'] {
    background: var(--accent-dim);
    color: var(--accent);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  :global(:root[data-theme='light']) .nav-item[data-active='true'] {
    color: var(--accent);
  }

  .settings-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 36px 32px 96px;
  }

  .settings-content {
    width: 100%;
    max-width: 720px;
    margin: 0;
  }
  .section-fade {
    display: flex;
    flex-direction: column;
    gap: 26px;
  }

  .search-results {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .no-results {
    margin: 0;
    padding: 8px 4px;
    color: var(--text-muted);
    font-size: 13px;
  }
  .result-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 11px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    text-align: left;
    transition:
      background var(--transition),
      border-color var(--transition);
  }
  .result-item:hover {
    background: var(--bg-hover);
    border-color: var(--accent-selected);
  }
  .result-icon {
    flex-shrink: 0;
    color: var(--text-secondary);
    display: flex;
  }
  .result-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .result-label {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .result-category {
    font-size: 11px;
    color: var(--text-muted);
  }

  :global(.settings-body .card-wrap),
  :global(.settings-body .row) {
    scroll-margin-top: 16px;
  }
  :global(.settings-body .setting-flash) {
    animation: setting-flash 1.3s ease;
    border-radius: var(--radius-md);
  }
  @keyframes setting-flash {
    0%,
    100% {
      box-shadow: 0 0 0 0 transparent;
    }
    12% {
      box-shadow: 0 0 0 3px var(--accent-selected);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    :global(.settings-body .setting-flash) {
      animation: none;
      box-shadow: 0 0 0 2px var(--accent-selected);
    }
  }

  @media (max-width: 900px) {
    .settings-layout {
      flex-direction: column;
    }
    .sidenav {
      width: 100%;
      padding: 10px 12px;
      border-right: none;
      border-bottom: 1px solid var(--border);
      gap: 8px;
      overflow-y: visible;
    }
    .nav-items {
      flex-direction: row;
      overflow-x: auto;
      gap: 4px;
      scrollbar-width: none;
      -ms-overflow-style: none;
    }
    .nav-items::-webkit-scrollbar {
      display: none;
    }
    .nav-item {
      flex-shrink: 0;
    }
    .settings-search {
      margin-bottom: 0;
    }
    .settings-body {
      padding: 24px 16px 84px;
    }
  }
</style>
