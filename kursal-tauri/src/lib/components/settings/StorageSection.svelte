<script lang="ts">
  import { onMount } from 'svelte';
  import { log } from '$lib/utils/log';
  import {
    Save,
    Trash2,
    RefreshCw,
    FolderOpen,
    Funnel,
    FolderSearch,
    ScrollText,
  } from 'lucide-svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { revealItemInDir } from '@tauri-apps/plugin-opener';
  import { isMobile } from '$lib/api/window';
  import { confirmDialog } from '$lib/state/confirm.svelte';
  import {
    listSharedFiles,
    revokeSharedFile,
    revokeSharedFilesBulk,
    getStorageUsage,
    type SharedFileEntry,
    type AutoAcceptConfig,
    type AutoAcceptMode,
    type AutoDownloadConfig,
    type AutoDownloadScope,
    type StorageUsage,
  } from '$lib/api/settings';
  import { settingsState } from '$lib/state/settings.svelte';
  import { settingsDirty } from '$lib/state/settingsDirty.svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { notifyError } from '$lib/utils/errors';
  import { flash } from '$lib/utils/flash.svelte';
  import Button from '$lib/components/Button.svelte';
  import SettingCard from './SettingCard.svelte';
  import SettingRow from './SettingRow.svelte';
  import Segmented from './Segmented.svelte';
  import Select from './Select.svelte';
  import Checkbox from './Checkbox.svelte';
  import TextInput from './TextInput.svelte';
  import LogViewerModal from './LogViewerModal.svelte';
  import { clearLogs } from '$lib/api/logs';
  import { t, dateLocale } from '$lib/i18n';

  let shared = $state<SharedFileEntry[]>([]);
  let sharedLoading = $state(false);
  let filterContact = $state<string>('');
  let filterNeverAccessed = $state(false);
  let selection = $state<Set<string>>(new Set());

  let acceptCfg = $state<AutoAcceptConfig>({ ...settingsState.autoAccept });
  let acceptSaving = $state(false);
  const acceptSaved = flash();
  const downloadSaved = flash();

  let downloadCfg = $state<AutoDownloadConfig>({
    ...settingsState.autoDownload,
  });
  let downloadSaving = $state(false);

  let usage = $state<StorageUsage | null>(null);
  let usageLoading = $state(false);
  let cfgInitialized = $state(settingsState.loaded);

  onMount(async () => {
    await loadShared();
    await settingsState.load();
    if (!cfgInitialized) {
      acceptCfg = { ...settingsState.autoAccept };
      downloadCfg = { ...settingsState.autoDownload };
      cfgInitialized = true;
    }
  });

  async function loadShared() {
    sharedLoading = true;
    try {
      shared = await listSharedFiles();
    } catch (e) {
      log.error(e);
    } finally {
      sharedLoading = false;
    }
  }

  async function loadUsage() {
    usageLoading = true;
    try {
      usage = await getStorageUsage();
    } catch (e) {
      log.error(e);
    } finally {
      usageLoading = false;
    }
  }

  const acceptDirty = $derived(
    acceptCfg.mode !== settingsState.autoAccept.mode ||
      acceptCfg.sizeCapBytes !== settingsState.autoAccept.sizeCapBytes
  );
  const downloadDirty = $derived(
    downloadCfg.scope !== settingsState.autoDownload.scope ||
      downloadCfg.limitBytes !== settingsState.autoDownload.limitBytes
  );

  $effect(() => {
    settingsDirty.value = acceptDirty || downloadDirty;
    return () => {
      settingsDirty.value = false;
    };
  });

  const filtered = $derived.by(() => {
    return shared.filter((f) => {
      if (filterContact && f.recipientId !== filterContact) return false;
      if (filterNeverAccessed && f.lastAccessedAt !== null) return false;
      return true;
    });
  });

  function toggleSelect(id: string, checked: boolean) {
    const next = new Set(selection);
    if (checked) next.add(id);
    else next.delete(id);
    selection = next;
  }

  function toggleSelectAll(checked: boolean) {
    if (checked) selection = new Set(filtered.map((f) => f.id));
    else selection = new Set();
  }

  async function handleRevoke(id: string) {
    try {
      await revokeSharedFile(id);
      shared = shared.filter((f) => f.id !== id);
      const next = new Set(selection);
      next.delete(id);
      selection = next;
    } catch (e) {
      notifyError(e);
    }
  }

  async function handleBulkRevoke() {
    if (selection.size === 0) return;
    const count = selection.size;
    const ok = await confirmDialog({
      title: t('settings.storage.revokeConfirmTitle', { count }),
      message: t('settings.storage.revokeConfirmMessage'),
      detail: t('settings.storage.revokeConfirmDetail'),
      confirmLabel: t('settings.storage.revokeConfirm'),
      tone: 'danger',
    });
    if (!ok) return;
    try {
      const ids = Array.from(selection);
      await revokeSharedFilesBulk(ids);
      shared = shared.filter((f) => !selection.has(f.id));
      selection = new Set();
    } catch (e) {
      notifyError(e);
    }
  }

  async function saveAccept() {
    acceptSaving = true;
    try {
      await settingsState.setAutoAccept({ ...acceptCfg });
      acceptSaved.trigger();
    } catch (e) {
      notifyError(e);
    } finally {
      acceptSaving = false;
    }
  }

  async function saveDownload() {
    downloadSaving = true;
    try {
      await settingsState.setAutoDownload({ ...downloadCfg });
      downloadSaved.trigger();
    } catch (e) {
      notifyError(e);
    } finally {
      downloadSaving = false;
    }
  }

  let logViewerOpen = $state(false);
  let logsClearing = $state(false);

  async function handleClearLogs() {
    const ok = await confirmDialog({
      title: t('settings.storage.clearLogsConfirmTitle'),
      message: t('settings.storage.clearLogsConfirmMessage'),
      detail: t('settings.storage.clearLogsConfirmDetail'),
      confirmLabel: t('settings.storage.clearLogsConfirm'),
      tone: 'danger',
    });
    if (!ok) return;
    logsClearing = true;
    try {
      const freed = await clearLogs();
      if (usage) usage = { ...usage, logsBytes: Math.max(0, usage.logsBytes - freed) };
    } catch (e) {
      notifyError(e);
    } finally {
      logsClearing = false;
    }
  }

  // Mobile app storage is sandboxed: no file manager can open these paths, so
  // the in-app viewer is the only way to read logs there.
  async function openLogs() {
    try {
      await invoke('open_log_folder');
    } catch (e) {
      notifyError(e);
    }
  }

  async function openFiles() {
    try {
      await invoke('open_files_folder');
    } catch (e) {
      notifyError(e);
    }
  }

  function fmtBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`;
    return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }

  function fmtDate(ts: number | null): string {
    if (ts === null) return t('settings.storage.never');
    return new Date(ts * 1000).toLocaleDateString(dateLocale());
  }

  function basename(p: string): string {
    const i = Math.max(p.lastIndexOf('/'), p.lastIndexOf('\\'));
    return i >= 0 ? p.slice(i + 1) : p;
  }

  function recipientLabel(id: string): string {
    return contactsState.getById(id)?.displayName ?? id;
  }

  function shortId(id: string): string {
    return id.length > 10 ? `${id.slice(0, 6)}…${id.slice(-4)}` : id;
  }
  function contactLabel(id: string): string {
    return contactsState.getById(id)?.displayName ?? shortId(id);
  }
  function colorFor(id: string): string {
    let h = 2166136261 >>> 0;
    for (let i = 0; i < id.length; i++) {
      h ^= id.charCodeAt(i);
      h = Math.imul(h, 16777619) >>> 0;
    }
    return `hsl(${h % 360} 65% 58%)`;
  }

  type Segment = {
    id: string;
    label: string;
    bytes: number;
    pct: number;
    color: string;
  };

  const dbSegments = $derived.by<Segment[]>(() => {
    if (!usage) return [];
    const total = Math.max(usage.dbBytes, 1);
    const segs: Segment[] = usage.perContact
      .filter((c) => c.dbBytes > 0)
      .map((c) => ({
        id: c.contactId,
        label: contactLabel(c.contactId),
        bytes: c.dbBytes,
        pct: (c.dbBytes / total) * 100,
        color: colorFor(c.contactId),
      }));
    const sum = segs.reduce((a, b) => a + b.bytes, 0);
    const systemBytes = Math.max(0, usage.dbBytes - sum);
    if (systemBytes > 0) {
      segs.push({
        id: '__system',
        label: t('settings.storage.systemLabel'),
        bytes: systemBytes,
        pct: (systemBytes / total) * 100,
        color: 'var(--text-muted)',
      });
    }
    segs.sort((a, b) => b.bytes - a.bytes);
    return segs;
  });

  const filesSegments = $derived.by<Segment[]>(() => {
    if (!usage) return [];
    const total = Math.max(usage.filesBytes, 1);
    return usage.perContact
      .filter((c) => c.filesBytes > 0)
      .map((c) => ({
        id: c.contactId,
        label: contactLabel(c.contactId),
        bytes: c.filesBytes,
        pct: (c.filesBytes / total) * 100,
        color: colorFor(c.contactId),
      }))
      .sort((a, b) => b.bytes - a.bytes);
  });

  async function handleReveal(filepath: string) {
    try {
      await revealItemInDir(filepath);
    } catch (e) {
      notifyError(e);
    }
  }

  function toMB(bytes: number): string {
    return String(Math.round(bytes / (1024 * 1024)));
  }
  function fromMB(mb: string): number {
    return Math.max(0, Math.floor(Number(mb) || 0)) * 1024 * 1024;
  }

  const acceptModes: { value: AutoAcceptMode; label: string }[] = [
    { value: 'nobody', label: t('settings.storage.acceptNobody') },
    { value: 'verified', label: t('settings.storage.acceptVerified') },
    { value: 'all', label: t('settings.storage.acceptAll') },
  ];

  const scopes: { value: AutoDownloadScope; label: string }[] = [
    { value: 'per_contact', label: t('settings.storage.scopePerContact') },
    { value: 'all_contacts', label: t('settings.storage.scopeAllContacts') },
  ];

  const contactOptions = $derived([
    { value: '', label: t('settings.storage.allContacts') },
    ...contactsState.contacts.map((c) => ({
      value: c.userId,
      label: c.displayName,
    })),
  ]);

  const allSelected = $derived(filtered.length > 0 && filtered.every((f) => selection.has(f.id)));
</script>

<div class="sec-head">
  <h2>{t('settings.storage.heading')}</h2>
  <p>{t('settings.storage.description')}</p>
</div>

<SettingCard title={t('settings.storage.sharedFilesCard')}>
  <div class="files-head">
    <div class="files-filters">
      <Select
        value={filterContact}
        options={contactOptions}
        onchange={(v) => (filterContact = v)}
        minWidth="180px"
      />
      <button
        type="button"
        class="chip"
        data-active={filterNeverAccessed}
        onclick={() => (filterNeverAccessed = !filterNeverAccessed)}
      >
        <Funnel size={12} />
        {t('settings.storage.neverAccessed')}
      </button>
    </div>
    <div class="files-actions">
      {#if selection.size > 0}
        <span class="selection-count"
          >{t('settings.storage.selectedCount', { count: selection.size })}</span
        >
        <button class="bulk-revoke-btn" onclick={handleBulkRevoke}>
          <Trash2 size={12} />
          {t('settings.storage.revokeButton')}
        </button>
      {/if}
      <button
        class="icon-btn"
        onclick={loadShared}
        disabled={sharedLoading}
        aria-label={t('settings.storage.refreshAriaLabel')}
      >
        <RefreshCw size={13} />
      </button>
    </div>
  </div>

  {#if sharedLoading}
    <div class="empty">{t('settings.storage.sharedFilesLoading')}</div>
  {:else if filtered.length === 0}
    <div class="empty">{t('settings.storage.noSharedFiles')}</div>
  {:else}
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th class="chk-col">
              <Checkbox
                checked={allSelected}
                onchange={toggleSelectAll}
                ariaLabel={t('settings.storage.selectAllAria')}
              />
            </th>
            <th>{t('settings.storage.tableFile')}</th>
            <th>{t('settings.storage.tableSize')}</th>
            <th>{t('settings.storage.tableRecipient')}</th>
            <th>{t('settings.storage.tableShared')}</th>
            <th>{t('settings.storage.tableLastAccess')}</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each filtered as f}
            <tr>
              <td class="chk-col">
                <Checkbox
                  checked={selection.has(f.id)}
                  onchange={(v) => toggleSelect(f.id, v)}
                  ariaLabel={t('settings.storage.selectFileAria', { file: basename(f.filepath) })}
                />
              </td>
              <td class="file-cell" title={f.filepath}>
                <span class="file-name">{basename(f.filepath)}</span>
              </td>
              <td class="nowrap">{fmtBytes(f.sizeBytes)}</td>
              <td class="ellipsis" title={f.recipientId}>{recipientLabel(f.recipientId)}</td>
              <td class="nowrap">{fmtDate(f.sharedAt)}</td>
              <td class="nowrap">{fmtDate(f.lastAccessedAt)}</td>
              <td class="row-actions">
                {#if !isMobile}
                  <button
                    class="icon-btn-sm"
                    onclick={() => handleReveal(f.filepath)}
                    aria-label={t('settings.storage.showInFolderAriaLabel')}
                    title={t('settings.storage.showInFolderTitle')}
                  >
                    <FolderSearch size={13} />
                  </button>
                {/if}
                <button class="revoke-btn" onclick={() => handleRevoke(f.id)}
                  >{t('settings.storage.revokeButton')}</button
                >
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</SettingCard>

<SettingCard
  title={t('settings.storage.autoAcceptCard')}
  description={t('settings.storage.autoAcceptDescription')}
>
  <SettingRow
    title={t('settings.storage.acceptFromRow')}
    description={t('settings.storage.acceptFromDescription')}
  >
    <Segmented
      value={acceptCfg.mode}
      options={acceptModes}
      onchange={(v) => (acceptCfg = { ...acceptCfg, mode: v })}
      size="sm"
    />
  </SettingRow>
  <SettingRow
    title={t('settings.storage.sizeCap')}
    description={t('settings.storage.sizeCapDescription')}
  >
    <div class="number-input">
      <TextInput
        type="number"
        min={0}
        width="84px"
        value={toMB(acceptCfg.sizeCapBytes)}
        onchange={(v) => (acceptCfg = { ...acceptCfg, sizeCapBytes: fromMB(v) })}
      />
      <span class="suffix">{t('settings.storage.mbSuffix')}</span>
    </div>
  </SettingRow>
  {#snippet footer()}
    <Button
      onclick={saveAccept}
      loading={acceptSaving}
      success={acceptSaved.active}
      disabled={!acceptDirty}
    >
      <Save size={13} />
      {t('settings.storage.saveButton')}
    </Button>
  {/snippet}
</SettingCard>

<SettingCard
  title={t('settings.storage.autoDownloadCard')}
  description={t('settings.storage.autoDownloadDescription')}
>
  <SettingRow
    title={t('settings.storage.scopeRow')}
    description={t('settings.storage.scopeDescription')}
  >
    <Segmented
      value={downloadCfg.scope}
      options={scopes}
      onchange={(v) => (downloadCfg = { ...downloadCfg, scope: v })}
      size="sm"
    />
  </SettingRow>
  <SettingRow
    title={t('settings.storage.limitRow')}
    description={t('settings.storage.limitDescription')}
  >
    <div class="number-input">
      <TextInput
        type="number"
        min={0}
        width="84px"
        value={toMB(downloadCfg.limitBytes)}
        onchange={(v) => (downloadCfg = { ...downloadCfg, limitBytes: fromMB(v) })}
      />
      <span class="suffix">{t('settings.storage.mbSuffix')}</span>
    </div>
  </SettingRow>
  {#snippet footer()}
    <Button
      onclick={saveDownload}
      loading={downloadSaving}
      success={downloadSaved.active}
      disabled={!downloadDirty}
    >
      <Save size={13} />
      {t('settings.storage.saveButton')}
    </Button>
  {/snippet}
</SettingCard>

<SettingCard title={t('settings.storage.diskUsageCard')}>
  {#if !usage}
    <div class="usage-cta">
      <p class="usage-cta-msg">
        {t('settings.storage.computeUsageMessage')}
      </p>
      <Button onclick={loadUsage} loading={usageLoading}>
        <RefreshCw size={13} />
        {t('settings.storage.computeUsageButton')}
      </Button>
    </div>
  {:else}
    <SettingRow
      title={t('settings.storage.logsRow')}
      description={t('settings.storage.logsDescription')}
    >
      <span class="usage-value">{fmtBytes(usage.logsBytes)}</span>
      <button class="clear-btn" onclick={handleClearLogs} disabled={logsClearing}>
        <Trash2 size={12} />
        {t('settings.storage.clearLogs')}
      </button>
    </SettingRow>

    <SettingRow
      title={t('settings.storage.avatarsRow')}
      description={t('settings.storage.avatarsDescription')}
    >
      <span class="usage-value">{fmtBytes(usage.avatarsBytes)}</span>
    </SettingRow>

    <div class="usage-stack">
      <div class="usage-row">
        <div class="usage-row-head">
          <span class="usage-row-title">{t('settings.storage.databaseSection')}</span>
          <span class="usage-row-total mono">{fmtBytes(usage.dbBytes)}</span>
        </div>
        <div class="bar" class:bar-empty={dbSegments.length === 0}>
          {#each dbSegments as s (s.id)}
            <div
              class="bar-seg"
              style="width: {s.pct}%; background: {s.color};"
              title="{s.label}: {fmtBytes(s.bytes)}"
            ></div>
          {/each}
        </div>
        {#if dbSegments.length > 0}
          <div class="legend">
            {#each dbSegments as s (s.id)}
              <div class="legend-item">
                <span class="dot" style="background: {s.color};"></span>
                <span class="legend-label ellipsis">{s.label}</span>
                <span class="mono legend-bytes">{fmtBytes(s.bytes)}</span>
              </div>
            {/each}
          </div>
        {/if}
      </div>

      <div class="usage-row">
        <div class="usage-row-head">
          <span class="usage-row-title">{t('settings.storage.fileSharesSection')}</span>
          <span class="usage-row-total mono">{fmtBytes(usage.filesBytes)}</span>
        </div>
        <div class="bar" class:bar-empty={filesSegments.length === 0}>
          {#each filesSegments as s (s.id)}
            <div
              class="bar-seg"
              style="width: {s.pct}%; background: {s.color};"
              title="{s.label}: {fmtBytes(s.bytes)}"
            ></div>
          {/each}
        </div>
        {#if filesSegments.length > 0}
          <div class="legend">
            {#each filesSegments as s (s.id)}
              <div class="legend-item">
                <span class="dot" style="background: {s.color};"></span>
                <span class="legend-label ellipsis">{s.label}</span>
                <span class="mono legend-bytes">{fmtBytes(s.bytes)}</span>
              </div>
            {/each}
          </div>
        {:else}
          <div class="legend muted-empty">{t('settings.storage.noSharedFilesLegend')}</div>
        {/if}
      </div>
    </div>
  {/if}
  {#snippet footer()}
    <button
      class="icon-btn"
      onclick={loadUsage}
      disabled={usageLoading}
      aria-label={t('settings.storage.refreshAriaLabel')}
    >
      <RefreshCw size={13} />
    </button>
    {#if !isMobile}
      <Button variant="secondary" onclick={openFiles}>
        <FolderOpen size={13} />
        {t('settings.storage.openFilesFolder')}
      </Button>
      <Button variant="secondary" onclick={openLogs}>
        <FolderOpen size={13} />
        {t('settings.storage.openLogFolder')}
      </Button>
    {/if}
    <Button variant="secondary" onclick={() => (logViewerOpen = true)}>
      <ScrollText size={13} />
      {t('settings.storage.viewLogs')}
    </Button>
  {/snippet}
</SettingCard>

{#if logViewerOpen}
  <LogViewerModal onClose={() => (logViewerOpen = false)} />
{/if}

<style>
  .files-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--border-light);
    flex-wrap: wrap;
  }
  .files-filters {
    display: flex;
    gap: 6px;
    align-items: center;
    flex-wrap: wrap;
  }
  .files-actions {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .selection-count {
    font-size: 12px;
    color: var(--text-muted);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 6px 10px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--bg-input);
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all var(--transition);
  }
  .chip:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .chip[data-active='true'] {
    background: var(--accent-dim);
    border-color: var(--accent);
    color: var(--accent);
  }

  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 7px;
    border-radius: var(--radius-md);
    background: var(--bg-input);
    border: 1px solid var(--border);
    color: var(--text-secondary);
    cursor: pointer;
    transition:
      background var(--transition),
      color var(--transition);
  }
  .icon-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .icon-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .empty {
    padding: 24px 14px;
    text-align: center;
    font-size: 13px;
    color: var(--text-muted);
  }

  .table-wrap {
    overflow-x: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }
  th,
  td {
    padding: 10px 12px;
    text-align: left;
    border-bottom: 1px solid var(--border-light);
  }
  tbody tr:last-child td {
    border-bottom: none;
  }
  th {
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-size: 10px;
  }
  td {
    color: var(--text-secondary);
  }
  .chk-col {
    width: 28px;
    padding-right: 0;
  }
  .ellipsis {
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file-cell {
    max-width: 280px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .file-name {
    color: var(--text-primary);
    font-weight: 500;
  }
  .nowrap {
    white-space: nowrap;
  }
  .row-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    justify-content: flex-end;
  }
  .icon-btn-sm {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 5px;
    border-radius: var(--radius-sm);
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-secondary);
    cursor: pointer;
    transition:
      background var(--transition),
      color var(--transition);
  }
  .icon-btn-sm:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .revoke-btn {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--danger);
    padding: 4px 10px;
    border-radius: var(--radius-sm);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all var(--transition);
  }
  .revoke-btn:hover {
    background: var(--danger-dim);
    border-color: rgba(248, 113, 113, 0.35);
  }
  .clear-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-secondary);
    padding: 4px 10px;
    border-radius: var(--radius-sm);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    transition: all var(--transition);
  }
  .clear-btn:hover:not(:disabled) {
    background: var(--danger-dim);
    border-color: rgba(248, 113, 113, 0.35);
    color: var(--danger);
  }
  .clear-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .bulk-revoke-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: var(--danger-dim);
    border: 1px solid rgba(248, 113, 113, 0.35);
    color: var(--danger);
    padding: 5px 10px;
    border-radius: var(--radius-sm);
    font-size: 11.5px;
    font-weight: 600;
    cursor: pointer;
    transition: all var(--transition);
  }
  .bulk-revoke-btn:hover {
    background: var(--danger);
    color: #fff;
    border-color: var(--danger);
  }

  .number-input {
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }
  .suffix {
    font-size: 12px;
    color: var(--text-muted);
  }

  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .usage-value {
    font-family: var(--font-mono);
    font-size: 13px;
    color: var(--text-primary);
  }

  .usage-stack {
    display: flex;
    flex-direction: column;
    padding: 14px;
    gap: 18px;
    border-top: 1px solid var(--border-light);
  }
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
    font-size: 12px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }
  .usage-row-total {
    font-size: 13px;
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
    font-size: 12px;
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
    font-size: 12px;
  }
  .usage-cta {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    padding: 24px 14px;
  }
  .usage-cta-msg {
    font-size: 13px;
    color: var(--text-muted);
    text-align: center;
    max-width: 320px;
  }
</style>
