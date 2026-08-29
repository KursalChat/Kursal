<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { Copy, Check, RotateCw, ScrollText } from 'lucide-svelte';
  import { copyText } from '$lib/utils/clipboard';
  import { listLogFiles, readLogTail, type LogFile } from '$lib/api/logs';
  import { notifyError } from '$lib/utils/errors';
  import { flash } from '$lib/utils/flash.svelte';
  import Modal from '$lib/components/Modal.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import Button from '$lib/components/Button.svelte';
  import { t } from '$lib/i18n';

  let { onClose }: { onClose: () => void } = $props();

  let files = $state<LogFile[]>([]);
  let selected = $state('');
  let text = $state('');
  let truncated = $state(false);
  let loading = $state(true);
  let failed = $state(false);
  let bodyEl = $state<HTMLPreElement | null>(null);
  const copied = flash();

  async function loadFile(path: string) {
    loading = true;
    try {
      const tail = await readLogTail(path);
      text = tail.text;
      truncated = tail.truncated;
      failed = false;
      // Scroll to the newest lines behind the overlay, so it never shows the
      // top of the file for a frame.
      await tick();
      if (bodyEl) bodyEl.scrollTop = bodyEl.scrollHeight;
    } catch (e) {
      failed = true;
      text = '';
      notifyError(e, 'settings.storage.logViewer.errorLoad');
    } finally {
      loading = false;
    }
  }

  async function loadAll() {
    loading = true;
    try {
      files = await listLogFiles();
      if (!files.length) {
        text = '';
        loading = false;
        return;
      }
      if (!files.some((f) => f.path === selected)) selected = files[0].path;
      await loadFile(selected);
    } catch (e) {
      failed = true;
      loading = false;
      notifyError(e, 'settings.storage.logViewer.errorLoad');
    }
  }

  async function copyAll() {
    if (!text) return;
    await copyText(text, { flash: copied });
  }

  onMount(loadAll);
</script>

<Modal
  title={t('settings.storage.logViewer.title')}
  {onClose}
  width={720}
  height="min(640px, 80vh, calc(100% - 32px))"
>
  <h2><ScrollText size={18} /> {t('settings.storage.logViewer.title')}</h2>

  <label class="file-picker" class:hidden={files.length < 2}>
    <span>{t('settings.storage.logViewer.fileLabel')}</span>
    <select
      value={selected}
      disabled={loading}
      onchange={(e) => {
        selected = e.currentTarget.value;
        void loadFile(selected);
      }}
    >
      {#each files as file (file.path)}
        <option value={file.path}>{file.name}</option>
      {/each}
    </select>
  </label>

  <!-- The pre stays mounted so refreshing swaps text under a fixed frame. -->
  <div class="log-wrap">
    <pre class="log-body selectable" bind:this={bodyEl}>{text}</pre>
    {#if loading || !text}
      <div class="log-overlay">
        {#if loading}
          <Spinner />
        {:else}
          <span class="muted">
            {#if !files.length}
              {t('settings.storage.logViewer.noFiles')}
            {:else if failed}
              {t('settings.storage.logViewer.errorLoad')}
            {:else}
              {t('settings.storage.logViewer.empty')}
            {/if}
          </span>
        {/if}
      </div>
    {/if}
  </div>

  <span class="notice">
    {truncated && !loading ? t('settings.storage.logViewer.truncated') : ''}
  </span>

  <div class="actions">
    <Button variant="secondary" onclick={loadAll} disabled={loading}>
      <RotateCw size={13} />
      {t('settings.storage.logViewer.refresh')}
    </Button>
    <Button variant="secondary" onclick={copyAll} disabled={loading || !text}>
      {#if copied.active}
        <Check size={13} />
        {t('common.copied')}
      {:else}
        <Copy size={13} />
        {t('common.copy')}
      {/if}
    </Button>
  </div>
  <button class="link" onclick={onClose}>{t('common.close')}</button>
</Modal>

<style>
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: var(--text-md);
    color: var(--text-primary);
  }
  .file-picker {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: var(--text-xs);
    color: var(--text-secondary);
  }
  /* Kept in the layout so a second log file appearing doesn't shift the pane. */
  .file-picker.hidden {
    visibility: hidden;
  }
  .file-picker select {
    flex: 1;
    min-width: 0;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-family: inherit;
    font-size: 12.5px;
    padding: 6px 8px;
  }
  .log-wrap {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .log-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-tertiary);
    border-radius: var(--radius-md);
  }
  .log-body {
    flex: 1;
    min-width: 0;
    overflow: auto;
    margin: 0;
    padding: 12px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text-secondary);
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11.5px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
    scrollbar-width: thin;
  }
  .muted {
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
  .notice {
    font-size: 11.5px;
    line-height: 1.4;
    min-height: 16px;
    color: var(--text-muted);
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .link {
    background: none;
    color: var(--text-secondary);
    font-size: var(--text-sm);
    align-self: center;
    padding: 4px 8px;
  }
  @media (hover: hover) {
    .link:hover {
      color: var(--text-primary);
    }
  }
</style>
