<script lang="ts">
  import { onMount } from 'svelte';
  import { save } from '@tauri-apps/plugin-dialog';
  import { mkdir, writeFile } from '@tauri-apps/plugin-fs';
  import { appCacheDir, join } from '@tauri-apps/api/path';
  import { Download, FileText, RefreshCw, Share2, SlidersHorizontal, Trash2 } from 'lucide-svelte';
  import Button from '$lib/components/Button.svelte';
  import LtcLimitsPicker from '$lib/components/LtcLimitsPicker.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import Toggle from '$lib/components/settings/Toggle.svelte';
  import { canShareFiles, shareAnchor, shareFile } from '$lib/api/share';
  import { ltcState } from '$lib/state/ltc.svelte';
  import { confirmDialog } from '$lib/state/confirm.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { t } from '$lib/i18n';
  import { parseError } from '$lib/utils/errors';
  import { log } from '$lib/utils/log';
  import type { LtcStatus } from '$lib/types';

  let { status }: { status: LtcStatus } = $props();

  const FILE_NAME = 'kursal-contact.kursal';
  const STALE_SECS = 15768000;

  let downloading = $state(false);
  let sharing = $state(false);
  let saving = $state(false);
  let editing = $state(false);
  let shareButton = $state<HTMLButtonElement | null>(null);
  let draftMaxUses = $state<number | null>(null);
  let draftTtl = $state<number | null>(null);
  let now = $state(Math.floor(Date.now() / 1000));

  onMount(() => {
    const timer = setInterval(() => (now = Math.floor(Date.now() / 1000)), 30000);
    return () => clearInterval(timer);
  });

  const expired = $derived(status.expiresAt !== null && now > status.expiresAt);
  const exhausted = $derived(status.maxUses !== null && status.uses >= status.maxUses);
  const dead = $derived(expired || exhausted);
  const stale = $derived(!dead && status.expiresAt === null && now - status.createdAt > STALE_SECS);
  const ageMonths = $derived(Math.floor((now - status.createdAt) / 2592000));
  const usePercent = $derived(
    status.maxUses ? Math.min(100, Math.round((status.uses / status.maxUses) * 100)) : 0
  );

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function pointerLabel(): string {
    if (!status.followRotations) return t('addContact.ltc.card.pointerDisabled');
    switch (status.pointerState) {
      case 'published':
        return t('addContact.ltc.card.pointerPublished');
      case 'failed':
        return t('addContact.ltc.card.pointerFailed');
      default:
        return t('addContact.ltc.card.pointerPending');
    }
  }

  async function handleFollowRotations(enabled: boolean) {
    try {
      await ltcState.setFollowRotations(enabled);
    } catch (e) {
      notifications.push(t('addContact.ltc.rotationError'), 'error');
      log.error('Toggling LTC rotation follow failed:', e);
    }
  }

  async function handleRepublish() {
    try {
      await ltcState.republishPointer();
    } catch (e) {
      notifications.push(t('addContact.ltc.republishError'), 'error');
      log.error('Republishing the LTC pointer failed:', e);
    }
  }

  function usesLabel(): string {
    return status.maxUses === null
      ? t('addContact.ltc.card.usesUnlimited', { used: status.uses })
      : t('addContact.ltc.card.usesOfMax', { used: status.uses, max: status.maxUses });
  }

  function expiryLabel(): string {
    if (exhausted) return t('addContact.ltc.card.exhausted');
    if (status.expiresAt === null) return t('addContact.ltc.card.noExpiry');

    const left = status.expiresAt - now;
    if (left <= 0) return t('addContact.ltc.card.expired');

    const days = Math.floor(left / 86400);
    if (days >= 1) return t('addContact.ltc.card.daysLeft', { n: days });

    const hours = Math.floor(left / 3600);
    if (hours >= 1) return t('addContact.ltc.card.hoursLeft', { n: hours });

    return t('addContact.ltc.card.minutesLeft', { n: Math.max(1, Math.floor(left / 60)) });
  }

  async function handleDownload() {
    downloading = true;
    try {
      const bytes = await ltcState.exportBytes();
      const path = await save({
        title: t('addContact.ltc.saveDialog'),
        defaultPath: FILE_NAME,
        filters: [
          {
            name: t('addContact.ltc.fileFilter'),
            extensions: ['kursal', 'application/octet-stream'],
          },
        ],
      });

      if (!path) {
        notifications.push(t('addContact.ltc.saveCancelled'), 'info');
        return;
      }

      await writeFile(path, bytes);
      notifications.push(t('addContact.ltc.exportSuccess'), 'success');
    } catch (e) {
      if (parseError(e).message.toLowerCase().includes('cancel')) {
        notifications.push(t('addContact.ltc.saveCancelled'), 'info');
      } else {
        notifications.push(t('addContact.ltc.exportError'), 'error');
      }
      log.error('Export failed:', e);
    } finally {
      downloading = false;
    }
  }

  async function handleShare() {
    sharing = true;
    try {
      const bytes = await ltcState.exportBytes();
      const dir = await appCacheDir();
      await mkdir(dir, { recursive: true });
      const path = await join(dir, FILE_NAME);
      await writeFile(path, bytes);
      await shareFile(
        path,
        'application/octet-stream',
        t('addContact.ltc.shareTitle'),
        shareButton ? shareAnchor(shareButton) : undefined
      );
    } catch (e) {
      notifications.push(t('addContact.ltc.shareError'), 'error');
      log.error('Share failed:', e);
    } finally {
      sharing = false;
    }
  }

  function startEditing() {
    draftMaxUses = status.maxUses;
    draftTtl = status.expiresAt === null ? null : status.expiresAt - status.createdAt;
    editing = true;
  }

  async function handleSaveLimits() {
    saving = true;
    try {
      await ltcState.updateLimits(draftMaxUses, draftTtl);
      editing = false;
      notifications.push(t('addContact.ltc.limitsSaved'), 'success');
    } catch (e) {
      notifications.push(t('addContact.ltc.limitsError'), 'error');
      log.error('Updating LTC limits failed:', e);
    } finally {
      saving = false;
    }
  }

  async function handleRegenerate() {
    const ok = await confirmDialog({
      title: t('addContact.ltc.regenerateConfirmTitle'),
      message: t('addContact.ltc.regenerateConfirmMessage'),
      confirmLabel: t('addContact.ltc.regenerateConfirmButton'),
      tone: 'danger',
    });
    if (!ok) return;

    const ttl = status.expiresAt === null ? null : status.expiresAt - status.createdAt;
    try {
      await ltcState.create(status.maxUses, ttl);
      notifications.push(t('addContact.ltc.regenerated'), 'success');
    } catch (e) {
      notifications.push(t('addContact.ltc.createError'), 'error');
      log.error('Regenerating the LTC failed:', e);
    }
  }

  async function handleRevoke() {
    const ok = await confirmDialog({
      title: t('addContact.ltc.revokeConfirmTitle'),
      message: t('addContact.ltc.revokeConfirmMessage'),
      confirmLabel: t('addContact.ltc.revokeConfirmButton'),
      tone: 'danger',
    });
    if (!ok) return;

    try {
      await ltcState.revoke();
      notifications.push(t('addContact.ltc.revoked'), 'success');
    } catch (e) {
      notifications.push(t('addContact.ltc.revokeError'), 'error');
      log.error('Revoking the LTC failed:', e);
    }
  }
</script>

<div class="card" class:dead>
  <div class="row">
    <span class="icon"><FileText size={17} /></span>

    <span class="meta">
      <strong>{FILE_NAME}</strong>
      <span class="sub">
        {formatSize(status.sizeBytes)}
        <span class="dot">·</span>
        {usesLabel()}
        <span class="dot">·</span>
        <span class:alert={dead}>{expiryLabel()}</span>
      </span>
    </span>

    {#if dead}
      <button
        class="icon-btn"
        title={t('addContact.ltc.card.regenerate')}
        onclick={handleRegenerate}
      >
        <RefreshCw size={15} />
      </button>
    {:else}
      <button
        class="icon-btn"
        title={t('addContact.ltc.card.download')}
        aria-label={t('addContact.ltc.card.download')}
        disabled={downloading}
        onclick={handleDownload}
      >
        {#if downloading}<Spinner size={14} color="currentColor" />{:else}<Download
            size={15}
          />{/if}
      </button>
      {#if canShareFiles}
        <button
          class="icon-btn"
          title={t('addContact.ltc.card.share')}
          aria-label={t('addContact.ltc.card.share')}
          disabled={sharing}
          bind:this={shareButton}
          onclick={handleShare}
        >
          {#if sharing}<Spinner size={14} color="currentColor" />{:else}<Share2 size={15} />{/if}
        </button>
      {/if}
    {/if}
  </div>

  {#if status.maxUses !== null}
    <div class="bar"><div class="bar-fill" style="width: {usePercent}%"></div></div>
  {/if}

  {#if !dead}
    <div class="rotation">
      <div class="rotation-row">
        <span class="rotation-label">
          <strong>{t('addContact.ltc.card.rotationRow')}</strong>
        </span>
        <Toggle
          checked={status.followRotations}
          disabled={ltcState.pointerBusy}
          ariaLabel={t('addContact.ltc.card.rotationRow')}
          onchange={handleFollowRotations}
        />
      </div>
      <p class="pointer" data-state={status.followRotations ? status.pointerState : 'disabled'}>
        {pointerLabel()}
        {#if status.followRotations && status.pointerState === 'failed'}
          <button type="button" disabled={ltcState.pointerBusy} onclick={handleRepublish}>
            {t('addContact.ltc.card.pointerRetry')}
          </button>
        {/if}
      </p>
    </div>
  {/if}

  {#if ltcState.reshareNeeded}
    <p class="hint">{t('addContact.ltc.card.reshareHint')}</p>
  {/if}

  {#if stale}
    <p class="hint">{t('addContact.ltc.card.staleHint', { n: ageMonths })}</p>
  {/if}

  {#if editing}
    <div class="editor">
      <LtcLimitsPicker bind:maxUses={draftMaxUses} bind:ttlSecs={draftTtl} disabled={saving} />
      <div class="editor-actions">
        <Button variant="primary" loading={saving} onclick={handleSaveLimits}>
          {t('addContact.ltc.card.saveLimits')}
        </Button>
        <Button variant="secondary" disabled={saving} onclick={() => (editing = false)}>
          {t('common.cancel')}
        </Button>
      </div>
    </div>
  {:else}
    <div class="links">
      <button type="button" onclick={startEditing}>
        <SlidersHorizontal size={12} />
        {t('addContact.ltc.card.editLimits')}
      </button>
      {#if !dead}
        <button type="button" onclick={handleRegenerate}>
          <RefreshCw size={12} />
          {t('addContact.ltc.card.regenerate')}
        </button>
      {/if}
      <button type="button" class="danger" onclick={handleRevoke}>
        <Trash2 size={12} />
        {t('addContact.ltc.card.revoke')}
      </button>
    </div>
  {/if}
</div>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    background: var(--bg-input);
    padding: 12px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .icon {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    flex-shrink: 0;
    border-radius: var(--radius-sm);
    background: var(--accent-dim);
    color: var(--accent);
  }

  .card.dead .icon {
    background: var(--bg-hover);
    color: var(--text-muted);
  }

  .meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    margin-right: auto;
  }

  .meta strong {
    font-size: var(--text-sm);
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: var(--text-xs);
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .dot {
    opacity: 0.5;
  }

  .alert {
    color: var(--danger);
    font-weight: 600;
  }

  .icon-btn {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    flex-shrink: 0;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--text-secondary);
    cursor: pointer;
    transition:
      background var(--transition),
      border-color var(--transition),
      color var(--transition);
  }

  .icon-btn:hover:not(:disabled) {
    background: var(--bg-hover);
    border-color: var(--accent-selected);
    color: var(--text-primary);
  }

  .icon-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .icon-btn:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .bar {
    height: 3px;
    border-radius: 2px;
    background: var(--bg-hover);
    overflow: hidden;
  }

  .bar-fill {
    height: 100%;
    background: var(--accent-solid);
    transition: width var(--transition);
  }

  .card.dead .bar-fill {
    background: var(--text-muted);
  }

  .rotation {
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-top: 1px solid var(--border-light);
    padding-top: 10px;
  }

  .rotation-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .rotation-label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    margin-right: auto;
  }

  .rotation-label strong {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-primary);
  }

  .pointer {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .pointer::before {
    content: '';
    width: 6px;
    height: 6px;
    flex-shrink: 0;
    border-radius: 50%;
    background: var(--text-muted);
  }

  .pointer[data-state='published']::before {
    background: var(--success);
  }

  .pointer[data-state='failed'] {
    color: var(--danger);
  }

  .pointer[data-state='failed']::before {
    background: var(--danger);
  }

  .pointer button {
    background: none;
    border: none;
    padding: 0;
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--accent);
    cursor: pointer;
    text-decoration: underline;
  }

  .pointer button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .hint {
    margin: 0;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--warning) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--warning) 35%, transparent);
    color: var(--text-secondary);
    font-size: var(--text-xs);
    line-height: 1.5;
  }

  .editor {
    display: flex;
    flex-direction: column;
    gap: 12px;
    border-top: 1px solid var(--border-light);
    padding-top: 12px;
  }

  .editor-actions {
    display: flex;
    gap: 8px;
  }

  .links {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    border-top: 1px solid var(--border-light);
    padding-top: 10px;
  }

  .links button {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: none;
    border: none;
    padding: 2px 0;
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .links button:hover {
    color: var(--text-primary);
  }

  .links button.danger:hover {
    color: var(--danger);
  }

  .links button:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: var(--radius-sm);
  }
</style>
