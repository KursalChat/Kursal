<script lang="ts">
  import { onMount } from 'svelte';
  import { slide } from 'svelte/transition';
  import { log } from '$lib/utils/log';
  import {
    Copy,
    ExternalLink,
    RefreshCw,
    ChevronDown,
    Eye,
    EyeOff,
    Activity,
    KeyRound,
    BookOpen,
  } from 'lucide-svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { copyText } from '$lib/utils/clipboard';
  import {
    checkForUpdates,
    generateLocalApiToken,
    getUpdaterEnabled,
    setUpdaterEnabled,
    getUpdateChannel,
    setUpdateChannel,
    getBackgroundMode,
    setBackgroundMode,
    type UpdateChannel,
    type LocalApiConfig,
  } from '$lib/api/settings';
  import { isMobile } from '$lib/api/window';
  import { settingsState } from '$lib/state/settings.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { notifyError } from '$lib/utils/errors';
  import { optimistic } from '$lib/utils/optimistic';
  import { clamp } from '$lib/utils/geometry';
  import { flash } from '$lib/utils/flash.svelte';
  import Button from '$lib/components/Button.svelte';
  import Benchmark from '$lib/components/Benchmark.svelte';
  import { listBenchmarks, type BenchmarkMeta } from '$lib/api/benchmark';
  import SettingCard from './SettingCard.svelte';
  import SettingRow from './SettingRow.svelte';
  import Toggle from './Toggle.svelte';
  import Segmented from './Segmented.svelte';
  import TextInput from './TextInput.svelte';
  import { t, dateLocale } from '$lib/i18n';
  import { TERMS_URL, termsDateLabel } from '$lib/utils/terms';

  let appVersion = $state('...');
  let checkingForUpdates = $state(false);
  let autoUpdater = $state(true);
  let updateChannel = $state<UpdateChannel>('stable');
  let backgroundMode = $state(true);
  let autostart = $state(false);
  let autostartSupported = $state(true);
  let autostartReady = $state(false);
  let autostartBusy = $state(false);

  const channelOptions = $derived([
    {
      value: 'stable' as UpdateChannel,
      label: t('settings.advanced.channelStable'),
    },
    {
      value: 'beta' as UpdateChannel,
      label: t('settings.advanced.channelBeta'),
    },
  ]);

  let api = $state<LocalApiConfig>({ ...settingsState.localApi });
  let apiSaving = $state(false);
  const copiedToken = flash();
  let apiInitialized = $state(settingsState.loaded);
  let newToken = $state<string | null>(null);
  let tokenVisible = $state(false);
  let generatingToken = $state(false);

  let benchmarks = $state<BenchmarkMeta[]>([]);
  let openId = $state<string | null>(null);
  let runningId = $state<string | null>(null);

  const BENCH_LABELS: Record<string, { nameKey: string; descKey: string }> = {
    otp: {
      nameKey: 'settings.advanced.benchmarkOtpName',
      descKey: 'settings.advanced.benchmarkOtpDescription',
    },
    otp_pow: {
      nameKey: 'settings.advanced.benchmarkOtpPowName',
      descKey: 'settings.advanced.benchmarkOtpPowDescription',
    },
    offline: {
      nameKey: 'settings.advanced.benchmarkOfflineName',
      descKey: 'settings.advanced.benchmarkOfflineDescription',
    },
  };

  function benchName(id: string): string {
    const key = BENCH_LABELS[id]?.nameKey;
    return key ? t(key) : id;
  }

  function benchDesc(id: string): string {
    const key = BENCH_LABELS[id]?.descKey;
    return key ? t(key) : '';
  }

  onMount(async () => {
    try {
      appVersion = `v${await getVersion()}`;
    } catch {
      appVersion = t('settings.advanced.versionUnknown');
    }
    try {
      autoUpdater = await getUpdaterEnabled();
    } catch {
      /* default true */
    }
    try {
      updateChannel = await getUpdateChannel();
    } catch {
      /* default stable */
    }
    try {
      backgroundMode = await getBackgroundMode();
    } catch {
      /* default true */
    }
    if (!isMobile) {
      try {
        const { isEnabled } = await import('@tauri-apps/plugin-autostart');
        autostart = await isEnabled();
      } catch (e) {
        log.error('Autostart plugin unavailable:', e);
        autostartSupported = false;
      } finally {
        autostartReady = true;
      }
    }
    await settingsState.load();
    if (!apiInitialized) {
      api = { ...settingsState.localApi };
      apiInitialized = true;
    }
    try {
      benchmarks = await listBenchmarks();
    } catch (e) {
      log.error(e);
    }
  });

  const apiDirty = $derived(
    api.enabled !== settingsState.localApi.enabled ||
      api.hostOnNetwork !== settingsState.localApi.hostOnNetwork ||
      api.port !== settingsState.localApi.port
  );

  async function handleCheckForUpdates() {
    checkingForUpdates = true;
    try {
      await checkForUpdates();
    } catch (e) {
      notifyError(e, 'settings.advanced.errorUpdateCheck');
    } finally {
      checkingForUpdates = false;
    }
  }

  async function handleAutoUpdater(value: boolean) {
    try {
      await optimistic(
        () => autoUpdater,
        (x) => (autoUpdater = x),
        setUpdaterEnabled,
        value
      );
    } catch (e) {
      notifyError(e);
    }
  }

  async function toggleAutostart(enabled: boolean) {
    autostartBusy = true;
    try {
      const { enable, disable } = await import('@tauri-apps/plugin-autostart');
      if (enabled) await enable();
      else await disable();
      autostart = enabled;
    } catch (e) {
      notifyError(e, 'settings.advanced.errorAutoStart');
    } finally {
      autostartBusy = false;
    }
  }

  async function handleBackgroundMode(value: boolean) {
    try {
      await optimistic(
        () => backgroundMode,
        (x) => (backgroundMode = x),
        setBackgroundMode,
        value
      );
    } catch (e) {
      notifyError(e);
    }
  }

  async function handleUpdateChannel(value: UpdateChannel) {
    try {
      await optimistic(
        () => updateChannel,
        (x) => (updateChannel = x),
        setUpdateChannel,
        value
      );
    } catch (e) {
      notifyError(e);
    }
  }

  async function saveApi() {
    apiSaving = true;
    try {
      const clean: LocalApiConfig = {
        ...api,
        port: clamp(Math.floor(api.port || 4892), 1, 65535),
      };
      await settingsState.setLocalApi(clean);
      api = { ...clean };
      notifications.push(t('settings.advanced.successApiSaved'), 'info');
    } catch (e) {
      notifyError(e);
    } finally {
      apiSaving = false;
    }
  }

  async function generateToken() {
    generatingToken = true;
    try {
      newToken = await generateLocalApiToken();
      tokenVisible = false;
      notifications.push(t('settings.advanced.successNewToken'), 'info');
    } catch (e) {
      notifyError(e);
    } finally {
      generatingToken = false;
    }
  }

  function hideToken() {
    newToken = null;
    tokenVisible = false;
  }

  async function copyToken() {
    if (!newToken) return;
    await copyText(newToken, {
      flash: copiedToken,
      errorKey: 'settings.advanced.errorCopyFailed',
    });
  }

  async function openLink(url: string) {
    try {
      await openUrl(url);
    } catch (e) {
      log.error(e);
      notifications.push(t('settings.advanced.errorOpenLink'), 'error');
    }
  }
</script>

<div class="sec-head">
  <h2>{t('settings.advanced.heading')}</h2>
  <p>{t('settings.advanced.description')}</p>
</div>

<SettingCard title={t('settings.advanced.aboutCard')}>
  <SettingRow title={t('settings.advanced.versionRow')}>
    <span class="value">Kursal {appVersion}</span>
  </SettingRow>
  <SettingRow title={t('settings.advanced.licenseRow')}>
    <span class="value">{t('settings.advanced.licenseValue')}</span>
  </SettingRow>
  <SettingRow title={t('settings.advanced.sourceCodeRow')}>
    <button class="link" onclick={() => openLink('https://kursal.chat/repository')}>
      {t('settings.advanced.sourceCodeButton')}
      <ExternalLink size={11} />
    </button>
  </SettingRow>
  <SettingRow title={t('settings.advanced.termsRow')} description={termsDateLabel(dateLocale())}>
    <button class="link" onclick={() => openLink(TERMS_URL)}>
      {t('settings.advanced.termsButton')}
      <ExternalLink size={11} />
    </button>
  </SettingRow>
</SettingCard>

{#if !isMobile}
  <SettingCard title={t('settings.advanced.backgroundCard')}>
    {#if autostartSupported}
      <SettingRow
        title={t('settings.advanced.autoStartRow')}
        description={t('settings.advanced.autoStartDescription')}
      >
        <Toggle
          checked={autostart}
          disabled={!autostartReady || autostartBusy}
          onchange={toggleAutostart}
          ariaLabel={t('settings.advanced.autoStartAriaLabel')}
        />
      </SettingRow>
    {/if}
    <SettingRow
      title={t('settings.advanced.backgroundRow')}
      description={t('settings.advanced.backgroundDescription')}
    >
      <Toggle
        checked={backgroundMode}
        onchange={handleBackgroundMode}
        ariaLabel={t('settings.advanced.backgroundAriaLabel')}
      />
    </SettingRow>
  </SettingCard>
{/if}

<SettingCard title={t('settings.advanced.updatesCard')}>
  <SettingRow
    title={t('settings.advanced.checkUpdatesRow')}
    description={t('settings.advanced.checkUpdatesDescription')}
  >
    <Button onclick={handleCheckForUpdates} loading={checkingForUpdates}>
      <RefreshCw size={13} />
      {t('settings.advanced.checkButton')}
    </Button>
  </SettingRow>
  <SettingRow
    title={isMobile
      ? t('settings.advanced.updateNoticesRow')
      : t('settings.advanced.autoUpdaterRow')}
    description={isMobile
      ? t('settings.advanced.updateNoticesDescription')
      : t('settings.advanced.autoUpdaterDescription')}
  >
    <Toggle
      checked={autoUpdater}
      onchange={handleAutoUpdater}
      ariaLabel={t('settings.advanced.autoUpdaterAriaLabel')}
    />
  </SettingRow>
  <SettingRow
    title={t('settings.advanced.channelRow')}
    description={t('settings.advanced.channelDescription')}
  >
    <Segmented
      value={updateChannel}
      options={channelOptions}
      onchange={handleUpdateChannel}
      size="sm"
    />
  </SettingRow>
</SettingCard>

<SettingCard
  title={t('settings.advanced.localApiCard')}
  description={t('settings.advanced.localApiDescription')}
>
  <SettingRow
    title={t('settings.advanced.enableRow')}
    description={t('settings.advanced.enableDescription')}
  >
    <Toggle
      checked={api.enabled}
      onchange={(v) => (api = { ...api, enabled: v })}
      ariaLabel={t('settings.advanced.enableAriaLabel')}
    />
  </SettingRow>
  {#if api.enabled}
    <SettingRow
      title={t('settings.advanced.hostOnNetworkRow')}
      description={t('settings.advanced.hostOnNetworkDescription')}
    >
      <Toggle
        checked={api.hostOnNetwork}
        onchange={(v) => (api = { ...api, hostOnNetwork: v })}
        ariaLabel={t('settings.advanced.hostOnNetworkAriaLabel')}
      />
    </SettingRow>
    <SettingRow title={t('settings.advanced.portRow')}>
      <TextInput
        type="number"
        min={1}
        max={65535}
        width="96px"
        value={String(api.port)}
        onchange={(v) => (api = { ...api, port: Number(v) || 4892 })}
      />
    </SettingRow>
    {#if settingsState.localApi.enabled}
      <SettingRow
        title={t('settings.advanced.openDocsRow')}
        description={t('settings.advanced.openDocsDescription')}
      >
        <Button
          variant="secondary"
          onclick={() => openLink(`http://localhost:${settingsState.localApi.port}`)}
        >
          <BookOpen size={13} />
          {t('settings.advanced.openDocsButton')}
        </Button>
      </SettingRow>
    {/if}
    <SettingRow
      title={t('settings.advanced.authTokenRow')}
      description={t('settings.advanced.authTokenDescription')}
    >
      <Button variant="secondary" loading={generatingToken} onclick={generateToken}>
        <KeyRound size={13} />
        {t('settings.advanced.newTokenButton')}
      </Button>
    </SettingRow>
    {#if newToken}
      <div class="token-display">
        <div class="token-head">
          <span class="token-label">{t('settings.advanced.tokenLabel')}</span>
          <button
            class="icon-btn"
            onclick={() => (tokenVisible = !tokenVisible)}
            aria-label={t('settings.advanced.toggleVisibilityAriaLabel')}
          >
            {#if tokenVisible}<EyeOff size={13} />{:else}<Eye size={13} />{/if}
          </button>
        </div>
        <code class="token-value mono selectable">
          {tokenVisible ? newToken : '•'.repeat(newToken.length)}
        </code>
        <div class="token-actions">
          <Button
            variant="secondary"
            onclick={copyToken}
            success={copiedToken.active}
            successLabel={t('common.copied')}
          >
            <Copy size={13} />
            {t('settings.advanced.copyTokenButton')}
          </Button>
          <Button variant="secondary" onclick={hideToken}
            >{t('settings.advanced.dismissTokenButton')}</Button
          >
        </div>
      </div>
    {/if}
  {/if}
  {#snippet footer()}
    <Button onclick={saveApi} loading={apiSaving} disabled={!apiDirty}
      >{t('settings.advanced.saveButton')}</Button
    >
  {/snippet}
</SettingCard>

<SettingCard
  title={t('settings.advanced.benchmarksCard')}
  description={t('settings.advanced.benchmarksDescription')}
>
  {#each benchmarks as bench (bench.id)}
    {@const isOpen = openId === bench.id}
    {@const isRunningThis = runningId === bench.id}
    {@const locked = runningId !== null}
    <div class="collapser" class:open={isOpen}>
      <button
        class="collapse-head"
        disabled={locked && !isRunningThis}
        onclick={() => {
          if (locked) return;
          openId = isOpen ? null : bench.id;
        }}
        aria-expanded={isOpen}
      >
        <div class="collapse-left">
          <Activity size={14} />
          <span>{benchName(bench.id)}</span>
        </div>
        <ChevronDown
          size={14}
          class="chev"
          style="transform: rotate({isOpen ? 0 : -90}deg); transition: transform 150ms ease"
        />
      </button>
      {#if isOpen}
        <div class="collapse-body" transition:slide={{ duration: 220 }}>
          <Benchmark
            id={bench.id}
            name={benchName(bench.id)}
            description={benchDesc(bench.id)}
            defaultIterations={bench.default_iterations}
            onRunningChange={(r) => (runningId = r ? bench.id : null)}
          />
        </div>
      {/if}
    </div>
  {/each}
</SettingCard>

<SettingCard title={t('settings.advanced.creditsCard')}>
  <ul class="credits">
    <li>
      <div class="credit-name">
        <button class="link" onclick={() => openLink('https://github.com/KodeurKubik')}>
          Kodeur_Kubik <ExternalLink size={11} />
        </button>
      </div>
      <span class="credit-role">{t('settings.advanced.creditCodingPaper')}</span>
    </li>
    <li>
      <div class="credit-name">
        <button class="link" onclick={() => openLink('https://github.com/Arlo-real')}>
          Arlo <ExternalLink size={11} />
        </button>
      </div>
      <span class="credit-role">{t('settings.advanced.creditPaper')}</span>
    </li>
    <li>
      <div class="credit-name">
        <button class="link" onclick={() => openLink('https://youtube.com/@ChoosingBerry')}>
          ChoosingBerry <ExternalLink size={11} />
        </button>
      </div>
      <span class="credit-role">{t('settings.advanced.creditArt')}</span>
    </li>
  </ul>
</SettingCard>

<style>
  .value {
    font-size: var(--text-sm);
    color: var(--text-primary);
    font-weight: 500;
  }

  .token-display {
    padding: 14px;
    border-top: 1px solid var(--border-light);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .token-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .token-label {
    font-size: var(--text-2xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--accent);
  }
  .token-value {
    display: block;
    background: var(--bg-input);
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    font-family: var(--font-mono);
    font-size: 11.5px;
    color: var(--text-primary);
    border: 1px solid var(--border);
    white-space: nowrap;
    overflow-x: auto;
    overflow-y: hidden;
    scrollbar-width: thin;
  }
  .token-actions {
    display: flex;
    gap: 6px;
  }
  .icon-btn {
    padding: 6px;
    border-radius: var(--radius-sm);
    background: var(--bg-input);
    border: 1px solid var(--border);
  }
  @media (hover: hover) {
    .icon-btn:hover {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
  }

  .collapser {
    display: flex;
    flex-direction: column;
    border-top: 1px solid var(--border-light);
    transition: background 150ms ease;
  }
  .collapser:first-child {
    border-top: none;
  }
  .collapser.open {
    background: var(--bg-tertiary);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .collapse-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 14px;
    background: none;
    border: none;
    color: var(--text-primary);
    cursor: pointer;
    font-size: var(--text-sm);
    font-weight: 600;
  }
  @media (hover: hover) {
    .collapse-head:hover:not(:disabled) {
      background: var(--bg-hover);
    }
  }
  .collapse-head:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  .collapse-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .collapse-body {
    padding: 12px 14px;
    border-top: 1px solid var(--border-light);
  }

  .credits {
    list-style: none;
    display: flex;
    flex-direction: column;
  }
  .credits li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border-light);
    font-size: var(--text-sm);
  }
  .credits li:last-child {
    border-bottom: none;
  }
  .credit-name {
    color: var(--text-primary);
    font-weight: 600;
  }
  .credit-role {
    color: var(--text-muted);
    font-size: var(--text-xs);
  }
  .link {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    color: var(--accent);
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }
  @media (hover: hover) {
    .link:hover {
      color: var(--accent-hover);
    }
  }
</style>
