<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { scale } from 'svelte/transition';
  import { Copy, Check, RotateCw, ScrollText } from 'lucide-svelte';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { listLogFiles, readLogTail, type LogFile } from '$lib/api/logs';
  import { trapFocus } from '$lib/utils/focusTrap';
  import { notifyError } from '$lib/utils/errors';
  import { flash } from '$lib/utils/flash.svelte';
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
    try {
      await writeText(text);
      copied.trigger();
    } catch (e) {
      notifyError(e);
    }
  }

  onMount(loadAll);
</script>

<div
  class="backdrop"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) onClose();
  }}
  onkeydown={(e) => {
    if (e.key === 'Escape') onClose();
  }}
>
  <div
    class="modal"
    in:scale={{ duration: 220, start: 0.94, opacity: 0 }}
    out:scale={{ duration: 160, start: 0.94, opacity: 0 }}
    role="dialog"
    aria-modal="true"
    aria-label={t('settings.storage.logViewer.title')}
    tabindex="-1"
    use:trapFocus
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
      <pre class="log-body" bind:this={bodyEl}>{text}</pre>
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
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: max(16px, var(--safe-top)) max(16px, var(--safe-right)) max(16px, var(--safe-bottom))
      max(16px, var(--safe-left));
  }
  .modal {
    background: var(--bg-secondary, var(--surface));
    border: 1px solid var(--border);
    border-radius: var(--radius-lg, 16px);
    padding: 22px;
    width: 100%;
    max-width: 720px;
    /* Fixed height: content length must not resize the dialog on refresh. */
    height: min(640px, 85vh, 100%);
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-shadow: 0 18px 50px rgba(0, 0, 0, 0.4);
  }
  h2 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: 16px;
    color: var(--text-primary);
  }
  .file-picker {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
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
    user-select: text;
  }
  .muted {
    color: var(--text-muted);
    font-size: 13px;
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
    font-size: 13px;
    align-self: center;
    padding: 4px 8px;
  }
  .link:hover {
    color: var(--text-primary);
  }
</style>
