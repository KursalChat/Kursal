<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { log } from '$lib/utils/log';
  import { listen } from '@tauri-apps/api/event';
  import {
    runBenchmark,
    cancelBenchmark,
    isBenchmarkRunning,
    type BenchmarkProgress,
    type BenchmarkResult,
  } from '$lib/api/benchmark';
  import { notifications } from '$lib/state/notifications.svelte';
  import { flash } from '$lib/utils/flash.svelte';
  import { copyText } from '$lib/utils/clipboard';
  import Button from '$lib/components/Button.svelte';
  import { Copy, CircleCheck, Check } from 'lucide-svelte';
  import { t } from '$lib/i18n';

  const copied = flash();

  let {
    id,
    name = '',
    description = '',
    defaultIterations = 100,
    onRunningChange = () => {},
  } = $props<{
    id: string;
    name?: string;
    description?: string;
    defaultIterations?: number;
    onRunningChange?: (running: boolean) => void;
  }>();

  let iterations = $state(100);
  let isRunning = $state(false);
  let progress = $state<BenchmarkProgress | null>(null);
  let result = $state<BenchmarkResult | null>(null);
  let error = $state<string | null>(null);
  let unlisten: () => void;

  onMount(async () => {
    iterations = defaultIterations;
    isRunning = await isBenchmarkRunning();

    const unlistenFn = await listen<BenchmarkProgress>('benchmark-progress', (event) => {
      progress = event.payload;
    });
    unlisten = unlistenFn;
  });

  onDestroy(() => {
    if (unlisten) unlisten();
  });

  async function start() {
    if (isRunning) return;
    isRunning = true;
    onRunningChange(true);
    progress = null;
    result = null;
    error = null;

    try {
      result = await runBenchmark(id, iterations);
    } catch (err) {
      error = String(err);
    } finally {
      isRunning = false;
      onRunningChange(false);
    }
  }

  async function cancel() {
    if (!isRunning) return;
    try {
      await cancelBenchmark();
    } catch (err) {
      log.error('Cancel failed:', err);
    }
  }

  async function copyResults() {
    if (!result) return;
    const text =
      `${name}\n` +
      `${t('settings.advanced.benchmarkResultIterations')}: ${result.iterations}\n` +
      `${t('settings.advanced.benchmarkResultTotalTime')}: ${(result.total_ms / 1000).toFixed(2)}s\n` +
      `${t('settings.advanced.benchmarkResultPerIterThreaded')}: ${result.average_with_threading_ms.toFixed(2)}ms\n` +
      `${t('settings.advanced.benchmarkResultPerIterCpu')}: ${result.average_per_iteration_ms.toFixed(2)}ms\n` +
      `${t('settings.advanced.benchmarkResultPerSecond')}: ${result.iterations_per_second.toFixed(2)}`;

    await copyText(text, { flash: copied, errorKey: 'settings.advanced.benchmarkCopyFailed' });
  }

  const pct = $derived(
    progress && progress.total > 0 ? Math.min(100, (progress.current / progress.total) * 100) : 0
  );
</script>

<div class="bench">
  <p class="bench-desc">{description}</p>

  <div class="bench-controls">
    <label class="iter-field">
      <span class="iter-label">{t('settings.advanced.benchmarkIterationsLabel')}</span>
      <input type="number" bind:value={iterations} min="1" max="10000" disabled={isRunning} />
    </label>

    <div class="bench-actions">
      <Button onclick={start} loading={isRunning}>
        {isRunning ? t('settings.advanced.benchmarkRunning') : t('settings.advanced.benchmarkRun')}
      </Button>
      {#if isRunning}
        <Button variant="danger" onclick={cancel}>
          {t('settings.advanced.benchmarkCancel')}
        </Button>
      {/if}
    </div>
  </div>

  {#if error}
    <div class="bench-error">
      {t('settings.advanced.benchmarkError', { error })}
    </div>
  {/if}

  {#if progress && isRunning}
    <div class="bench-progress">
      <div class="prog-meta">
        <span>
          {t('settings.advanced.benchmarkProgress', {
            current: progress.current,
            total: progress.total,
          })}
        </span>
        <span>
          {t('settings.advanced.benchmarkElapsed', {
            seconds: (progress.elapsed_ms / 1000).toFixed(1),
          })}
        </span>
      </div>
      <div class="prog-track">
        <div class="prog-fill" style="width: {pct}%"></div>
      </div>
    </div>
  {/if}

  {#if result}
    <div class="bench-results">
      <div class="results-head">
        <div class="results-title">
          <CircleCheck size={14} />
          <span>{t('settings.advanced.benchmarkResultsTitle')}</span>
        </div>
        <button
          class="copy-btn"
          class:confirmed={copied.active}
          onclick={copyResults}
          aria-label={copied.active
            ? t('common.copied')
            : t('settings.advanced.benchmarkCopyResults')}
          title={t('settings.advanced.benchmarkCopyResults')}
        >
          {#if copied.active}
            <Check size={13} />
          {:else}
            <Copy size={13} />
          {/if}
        </button>
      </div>

      <div class="results-grid">
        <div class="stat">
          <div class="stat-label">
            {t('settings.advanced.benchmarkResultIterations')}
          </div>
          <div class="stat-value">{result.iterations}</div>
        </div>
        <div class="stat">
          <div class="stat-label">
            {t('settings.advanced.benchmarkResultTotalTime')}
          </div>
          <div class="stat-value">
            {(result.total_ms / 1000).toFixed(2)}<span class="unit">s</span>
          </div>
        </div>
        <div class="stat">
          <div class="stat-label">
            {t('settings.advanced.benchmarkResultPerIterThreaded')}
          </div>
          <div class="stat-value">
            {result.average_with_threading_ms.toFixed(2)}<span class="unit">ms</span>
          </div>
        </div>
        <div class="stat">
          <div class="stat-label">
            {t('settings.advanced.benchmarkResultPerIterCpu')}
          </div>
          <div class="stat-value">
            {result.average_per_iteration_ms.toFixed(2)}<span class="unit">ms</span>
          </div>
        </div>
        <div class="stat">
          <div class="stat-label">
            {t('settings.advanced.benchmarkResultPerSecond')}
          </div>
          <div class="stat-value">
            {result.iterations_per_second.toFixed(2)}<span class="unit">/s</span>
          </div>
        </div>
      </div>
    </div>
  {/if}
</div>

<style>
  .bench {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 12px;
  }

  .bench-desc {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
    line-height: 1.5;
  }

  .bench-controls {
    display: flex;
    align-items: flex-end;
    gap: 12px;
    flex-wrap: wrap;
  }

  .iter-field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 120px;
  }

  .iter-label {
    font-size: var(--text-2xs);
    font-weight: 600;
    color: var(--text-secondary);
    letter-spacing: 0.02em;
  }

  .iter-field input {
    padding: 7px 10px;
    font-size: var(--text-sm);
    background: var(--bg-input);
    border: 1px solid var(--border);
    color: var(--text-primary);
    border-radius: var(--radius-sm);
    width: 100%;
    transition: border-color var(--transition);
  }

  .iter-field input:focus {
    outline: none;
    border-color: var(--accent);
  }

  .iter-field input:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .bench-actions {
    display: flex;
    gap: 8px;
  }

  .bench-error {
    padding: 8px 10px;
    background: var(--danger-dim, rgba(239, 68, 68, 0.12));
    color: var(--danger, #ef4444);
    border-radius: var(--radius-sm);
    font-size: var(--text-xs);
  }

  .bench-progress {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
  }

  .prog-meta {
    display: flex;
    justify-content: space-between;
    font-size: 11.5px;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }

  .prog-track {
    height: 6px;
    background: var(--bg-hover);
    border-radius: var(--radius-md);
    overflow: hidden;
  }

  .prog-fill {
    height: 100%;
    background: var(--accent);
    border-radius: inherit;
    transition: width 120ms linear;
  }

  .bench-results {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
  }

  .results-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .results-title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--text-2xs);
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-primary);
  }

  .results-title :global(svg) {
    color: var(--success);
  }

  .copy-btn {
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
    transition:
      background var(--transition),
      color var(--transition);
  }

  @media (hover: hover) {
    .copy-btn:hover {
      color: var(--text-primary);
      background: var(--bg-hover);
    }
  }

  .copy-btn.confirmed {
    color: var(--success);
  }
  @media (hover: hover) {
    .copy-btn.confirmed:hover {
      color: var(--success);
    }
  }

  .results-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(140px, 1fr));
    gap: 10px;
  }

  .stat {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px;
    background: var(--bg-tertiary);
    border-radius: var(--radius-sm);
    border: 1px solid var(--border-light);
  }

  .stat-label {
    font-size: 10.5px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    font-weight: 600;
  }

  .stat-value {
    font-size: var(--text-md);
    font-family: var(--font-mono);
    font-weight: 500;
    color: var(--text-primary);
  }

  .unit {
    font-size: var(--text-2xs);
    color: var(--text-muted);
    margin-left: 2px;
    font-weight: 400;
  }
</style>
