<script lang="ts">
  import { onMount } from 'svelte';
  import { log } from '$lib/utils/log';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import { readFile } from '@tauri-apps/plugin-fs';
  import { importLtc } from '$lib/api/ltc';
  import { notifications } from '$lib/state/notifications.svelte';
  import { parseError } from '$lib/utils/errors';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { ltcState } from '$lib/state/ltc.svelte';
  import { goto } from '$app/navigation';
  import Button from '$lib/components/Button.svelte';
  import LtcCard from '$lib/components/LtcCard.svelte';
  import LtcLimitsPicker from '$lib/components/LtcLimitsPicker.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { t } from '$lib/i18n';
  import { Plus, Upload, ShieldAlert, FolderOpen, FileCheckCorner } from 'lucide-svelte';

  let creating = $state(false);
  let newMaxUses = $state<number | null>(null);
  let newTtl = $state<number | null>(2592000);
  let importing = $state(false);
  let importError = $state('');
  let dragging = $state(false);

  let unlistenPromises: Array<Promise<() => void>> = [];

  onMount(() => {
    void ltcState.init();

    unlistenPromises.push(
      listen<{ paths: string[] }>('tauri://drag-enter', () => {
        dragging = true;
      })
    );
    unlistenPromises.push(
      listen<{ paths: string[] }>('tauri://drag-leave', () => {
        dragging = false;
      })
    );
    unlistenPromises.push(
      listen<{ paths: string[] }>('tauri://drag-drop', async (event) => {
        dragging = false;
        if (event.payload.paths && event.payload.paths.length > 0) {
          await handleImportPath(event.payload.paths[0]);
        }
      })
    );

    return () => {
      unlistenPromises.forEach((p) => p.then((unlisten) => unlisten()));
    };
  });

  async function handleImportPath(path: string) {
    if (!path.endsWith('.kursal')) {
      importError = t('addContact.ltc.invalidFileType');
      return;
    }
    await runImport(async () => Array.from(await readFile(path)));
  }

  async function runImport(load: () => Promise<number[]>) {
    importing = true;
    importError = '';
    try {
      const contact = await importLtc(await load());
      contactsState.upsert(contact);
      notifications.push(t('addContact.ltc.importSuccess'), 'success');
      goto(`/chat/${contact.userId}`);
    } catch (e) {
      const errMsg = parseError(e).message;
      if (errMsg.includes('expired')) {
        importError = t('addContact.ltc.expiredError');
      } else if (errMsg.toLowerCase().includes('already used')) {
        importError = t('addContact.ltc.alreadyUsedError');
      } else if (errMsg.toLowerCase().includes('yourself')) {
        importError = t('addContact.ltc.selfError');
      } else {
        importError = t('addContact.ltc.invalidFileError');
      }
      log.error('Import failed:', e);
    } finally {
      importing = false;
    }
  }

  async function handleCreate() {
    creating = true;
    try {
      await ltcState.create(newMaxUses, newTtl);
      notifications.push(t('addContact.ltc.created'), 'success');
    } catch (e) {
      notifications.push(t('addContact.ltc.createError'), 'error');
      log.error('Creating the LTC failed:', e);
    } finally {
      creating = false;
    }
  }

  async function handleImportSelectedFile(file: File) {
    if (!file.name.endsWith('.kursal')) {
      importError = t('addContact.ltc.invalidFileType');
      return;
    }
    await runImport(async () => Array.from(new Uint8Array(await file.arrayBuffer())));
  }

  async function handleImportFile(event: Event) {
    const input = event.target as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;

    await handleImportSelectedFile(file);
    input.value = '';
  }

  let fileInput = $state<HTMLInputElement | null>(null);

  async function handleDropZoneClick() {
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        pickerMode: 'document',
        fileAccessMode: 'copy',
        filters: [
          {
            name: 'Kursal data file',
            extensions: ['kursal', 'application/octet-stream'],
          },
        ],
      });

      if (selected) {
        const selectedPath = Array.isArray(selected) ? selected[0] : selected;
        const path =
          typeof selectedPath === 'string'
            ? selectedPath
            : String((selectedPath as { path?: string }).path ?? selectedPath);
        await handleImportPath(path);
        return;
      }
    } catch (err) {
      log.warn(t('addContact.ltc.pickerUnavailable'), err);
    }

    fileInput?.click();
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragging = true;
  }

  function handleDragLeave(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragging = false;
  }

  async function handleDrop(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragging = false;
    const files = e.dataTransfer?.files;
    if (files && files.length > 0) {
      await handleImportSelectedFile(files[0]);
    }
  }

  function handleDropZoneKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter' || e.key === ' ') {
      e.preventDefault();
      handleDropZoneClick();
    }
  }
</script>

<div class="ltc-flow">
  <section class="mode-content">
    <div class="heading-row">
      <div>
        <h3>{t('addContact.ltc.createTitle')}</h3>
      </div>
    </div>

    <p class="explanation">
      {t('addContact.ltc.createDescription')}
    </p>

    {#if ltcState.loading}
      <div class="loading"><Spinner size={18} /></div>
    {:else if ltcState.status}
      <LtcCard status={ltcState.status} />
    {:else}
      <LtcLimitsPicker bind:maxUses={newMaxUses} bind:ttlSecs={newTtl} disabled={creating} />
      <Button variant="primary" loading={creating} onclick={handleCreate}>
        <Plus size={14} />
        {t('addContact.ltc.createButton')}
      </Button>
    {/if}

    <div class="warning">
      <ShieldAlert size={16} />
      <div>
        <strong>{t('addContact.ltc.warningTitle')}</strong>
        <p>
          {t('addContact.ltc.warningDescription')}
        </p>
      </div>
    </div>

    <div class="warning">
      <ShieldAlert size={16} />
      <div>
        <strong>{t('addContact.ltc.securityNoteTitle')}</strong>
        <p>
          {t('addContact.ltc.securityNoteDescription')}
        </p>
      </div>
    </div>
  </section>

  <div class="divider" role="separator" aria-hidden="true">
    <span>{t('addContact.otp.orDivider')}</span>
  </div>

  <section class="mode-content">
    <div class="heading-row">
      <div>
        <h3>{t('addContact.ltc.importTitle')}</h3>
        <p class="subtle">{t('addContact.ltc.importSubtitle')}</p>
      </div>
    </div>

    <div
      class="drop-zone"
      class:dragging
      ondragover={handleDragOver}
      ondragleave={handleDragLeave}
      ondrop={handleDrop}
      onclick={handleDropZoneClick}
      onkeydown={handleDropZoneKeydown}
      role="button"
      tabindex="0"
      aria-label={t('addContact.ltc.dropZoneAriaLabel')}
    >
      {#if importing}
        <Upload size={28} />
      {:else if dragging}
        <FileCheckCorner size={28} />
      {:else}
        <FolderOpen size={28} />
      {/if}
      <p>{dragging ? t('addContact.ltc.releaseToImport') : t('addContact.ltc.dropFile')}</p>
      <span>{t('addContact.ltc.orBrowse')}</span>
    </div>

    <input
      type="file"
      accept=".kursal"
      onchange={handleImportFile}
      bind:this={fileInput}
      style="display: none"
      disabled={importing}
    />

    {#if importError}
      <div class="error-message">
        {importError}
      </div>
    {/if}
  </section>
</div>

<style>
  .ltc-flow {
    max-width: 760px;
    display: grid;
    gap: 16px;
  }

  .mode-content {
    display: flex;
    flex-direction: column;
    gap: 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    padding: 18px;
  }

  .heading-row {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 14px;
  }

  h3 {
    margin: 0;
    font-size: 18px;
    font-weight: 700;
    line-height: 1.2;
    color: var(--text-primary);
  }

  .subtle {
    margin: 4px 0 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .explanation {
    margin: 0;
    color: var(--text-secondary);
    font-size: 13px;
    line-height: 1.5;
  }

  .loading {
    display: grid;
    place-items: center;
    padding: 24px 0;
  }

  .warning {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    background: color-mix(in srgb, var(--warning) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--warning) 35%, transparent);
    border-radius: var(--radius-md);
    padding: 12px;
    font-size: 13px;
    line-height: 1.5;
    color: var(--warning);
  }

  .warning strong {
    color: var(--warning);
  }

  .warning p {
    margin: 4px 0 0;
    color: var(--text-secondary);
  }

  .divider {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    padding: 4px 0;
  }

  .divider::before,
  .divider::after {
    content: '';
    flex: 1;
    height: 1px;
    background: var(--border);
  }

  .drop-zone {
    border: 1px dashed var(--border);
    border-radius: var(--radius-md);
    padding: 28px 16px;
    text-align: center;
    cursor: pointer;
    transition:
      border-color var(--transition),
      background var(--transition);
    background: var(--bg-input);
    width: 100%;
    color: var(--text-secondary);
  }

  .drop-zone:hover,
  .drop-zone.dragging {
    border-color: var(--accent);
    background: var(--accent-dim);
    color: var(--text-primary);
  }

  .drop-zone:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .drop-zone p {
    margin: 0;
    color: var(--text-primary);
    font-weight: 600;
  }

  .drop-zone span {
    display: block;
    margin-top: 4px;
    font-size: 13px;
    color: var(--text-muted);
  }

  .error-message {
    background: var(--danger-dim);
    color: var(--danger);
    padding: 12px;
    border-radius: var(--radius-md);
    font-size: 13px;
    line-height: 1.5;
    border: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
  }

  :global(.mode-content > .button) {
    width: 100%;
  }

  @media (max-width: 740px) {
    .mode-content {
      padding: 16px;
    }
  }
</style>
