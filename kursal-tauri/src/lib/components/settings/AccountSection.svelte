<script lang="ts">
  import { onMount } from 'svelte';
  import { log } from '$lib/utils/log';
  import {
    Upload,
    Download,
    FolderOpen,
    Trash2,
    Save,
    Bell,
    BellRing,
    Copy,
    Check,
  } from 'lucide-svelte';
  import { save as saveDialog, open as openDialog } from '@tauri-apps/plugin-dialog';
  import { writeFile, readFile } from '@tauri-apps/plugin-fs';
  import { broadcastProfile, setLocalUserAvatar } from '$lib/api/identity';
  import { exportBackup, importBackup } from '$lib/api/backup';
  import {
    ensurePermission,
    getPermission,
    sendTestNotification,
  } from '$lib/state/systemNotify.svelte';
  import { profileState } from '$lib/state/profile.svelte';
  import { prefsState, type NotificationPreview, type DndSchedule } from '$lib/state/prefs.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { notifyError } from '$lib/utils/errors';
  import { flash } from '$lib/utils/flash.svelte';
  import { withAvatarCacheBust } from '$lib/utils/avatarUrl';
  import {
    DISPLAY_NAME_MAX,
    validateAvatarBytes,
    validateDisplayName,
  } from '$lib/utils/displayName';
  import { confirmDialog } from '$lib/state/confirm.svelte';
  import Avatar from '$lib/components/Avatar.svelte';
  import AvatarPicker from '$lib/components/AvatarPicker.svelte';
  import Button from '$lib/components/Button.svelte';
  import SettingCard from './SettingCard.svelte';
  import SettingRow from './SettingRow.svelte';
  import Toggle from './Toggle.svelte';
  import Segmented from './Segmented.svelte';
  import TextInput from './TextInput.svelte';
  import DndDial from './DndDial.svelte';
  import { copyText } from '$lib/utils/clipboard';
  import { t } from '$lib/i18n';

  let displayName = $state('You');
  let avatarSrc = $state<string | null>(null);
  let pendingAvatarBytes = $state<number[] | null | undefined>(undefined);
  let savingProfile = $state(false);
  let exporting = $state(false);
  let importing = $state(false);

  let exportOpen = $state(false);
  const profileSaved = flash();
  const backupSaved = flash();
  const copiedUserId = flash();
  let exportPwd = $state('');
  let exportPwd2 = $state('');

  let importOpen = $state(false);
  let importPwd = $state('');
  let pendingImportBytes = $state<Uint8Array | null>(null);

  let preview = $state<NotificationPreview>('content');
  let dnd = $state<DndSchedule>({
    enabled: false,
    start: '22:00',
    end: '06:00',
  });

  let notifPermission = $state<boolean>(false);
  let requestingPerm = $state(false);
  let sendingTest = $state(false);

  const profileDirty = $derived(
    displayName.trim() !== profileState.displayName.trim() || pendingAvatarBytes !== undefined
  );

  // Mirrors the backend's ProfileInfo::validate so the user sees the problem
  // before a broadcast round-trip rejects it.
  const PROFILE_ERROR_KEYS = {
    empty: 'settings.account.errorNameEmpty',
    length: 'settings.account.errorNameLength',
    edgeWhitespace: 'settings.account.errorNameEdgeWhitespace',
    unsupportedChars: 'settings.account.errorNameUnsupportedChars',
    avatarTooLarge: 'settings.account.errorAvatarTooLarge',
  } as const;

  // Validate the trimmed value: surrounding spaces are dropped on save rather
  // than reported back to the user as an error.
  const profileError = $derived(
    validateDisplayName(displayName.trim()) ?? validateAvatarBytes(pendingAvatarBytes)
  );
  const profileErrorText = $derived(profileError ? t(PROFILE_ERROR_KEYS[profileError]) : '');

  onMount(async () => {
    await profileState.load();
    displayName = profileState.displayName;
    avatarSrc = profileState.avatarPath;
    pendingAvatarBytes = undefined;
    preview = prefsState.notificationPreview;
    dnd = { ...prefsState.dnd };
    notifPermission = await getPermission(true);
  });

  function handleAvatarChange(dataUrl: string, bytes: number[]) {
    avatarSrc = dataUrl;
    pendingAvatarBytes = bytes;
  }

  async function saveProfile() {
    if (profileError) {
      notifications.push(profileErrorText, 'error');
      return;
    }
    const nameToSave = displayName.trim();
    savingProfile = true;
    try {
      avatarSrc = await profileState.save(nameToSave, pendingAvatarBytes);
      pendingAvatarBytes = undefined;
      profileSaved.trigger();
    } catch (e) {
      notifyError(e, 'settings.account.errorBroadcastFailed');
    } finally {
      savingProfile = false;
    }
  }

  function resetProfile() {
    displayName = profileState.displayName;
    avatarSrc = profileState.avatarPath;
    pendingAvatarBytes = undefined;
  }

  function resetExport() {
    exportOpen = false;
    exportPwd = '';
    exportPwd2 = '';
  }

  function resetImport() {
    importOpen = false;
    importPwd = '';
    pendingImportBytes = null;
  }

  async function handleExportBackup() {
    if (exportPwd.length < 8) {
      notifications.push(t('settings.account.errorPasswordTooShort'), 'error');
      return;
    }
    if (exportPwd !== exportPwd2) {
      notifications.push(t('settings.account.errorPasswordMismatch'), 'error');
      return;
    }
    exporting = true;
    try {
      const bytes = await exportBackup(exportPwd);
      const path = await saveDialog({
        defaultPath: 'backup.kursal',
        filters: [{ name: 'Kursal Backup', extensions: ['kursal'] }],
      });
      if (!path) return;
      await writeFile(path, new Uint8Array(bytes));
      // resetExport() collapses this panel, so the flash lands on the export
      // button that takes its place in the row.
      backupSaved.trigger();
      resetExport();
    } catch (e) {
      notifyError(e, 'settings.account.errorExport');
    } finally {
      exporting = false;
    }
  }

  async function pickImportFile() {
    try {
      const picked = await openDialog({
        multiple: false,
        filters: [{ name: 'Kursal Backup', extensions: ['kursal'] }],
      });
      if (!picked || Array.isArray(picked)) return;
      pendingImportBytes = await readFile(picked as string);
      importOpen = true;
    } catch (e) {
      notifyError(e, 'settings.account.errorReadFailed');
    }
  }

  async function handleImportBackup() {
    if (!pendingImportBytes) return;
    if (importPwd.length === 0) {
      notifications.push(t('settings.account.errorEnterPassword'), 'error');
      return;
    }
    const confirmed = await confirmDialog({
      title: t('settings.account.replaceConfirmTitle'),
      message: t('settings.account.replaceConfirmMessage'),
      confirmLabel: t('settings.account.replaceConfirmLabel'),
      cancelLabel: t('common.cancel'),
      tone: 'danger',
      holdMs: 5000,
    });
    if (!confirmed) return;
    importing = true;
    try {
      await importBackup(Array.from(pendingImportBytes), importPwd);
      notifications.push(t('settings.account.successBackupImported'), 'success');
      resetImport();
    } catch (e) {
      notifyError(e, 'settings.account.errorImport');
    } finally {
      importing = false;
    }
  }

  function updatePreview(value: NotificationPreview) {
    preview = value;
    prefsState.setPreview(value);
  }

  function updateDnd(changes: Partial<DndSchedule>) {
    dnd = { ...dnd, ...changes };
    prefsState.setDnd(dnd);
  }

  async function handleEnableNotifications() {
    requestingPerm = true;
    try {
      const ok = await ensurePermission();
      notifPermission = ok;
      if (!ok) {
        notifications.push(t('settings.account.errorPermissionDenied'), 'error');
      }
    } finally {
      requestingPerm = false;
    }
  }

  async function handleTestNotification() {
    sendingTest = true;
    try {
      const sent = await sendTestNotification();
      notifPermission = await getPermission(true);
      if (!sent) {
        notifications.push(t('settings.account.errorPermissionDenied'), 'error');
      }
    } finally {
      sendingTest = false;
    }
  }

  async function copyUserId() {
    if (!profileState.userId) return;
    await copyText(profileState.userId, {
      flash: copiedUserId,
      errorKey: 'settings.account.errorCopyFailed',
    });
  }
</script>

<div class="sec-head">
  <h2>{t('settings.account.heading')}</h2>
  <p>{t('settings.account.description')}</p>
</div>

<SettingCard title={t('settings.account.profileCard')}>
  <div class="profile">
    <AvatarPicker onChange={handleAvatarChange}>
      {#snippet children(open)}
        <div class="avatar-wrap">
          <Avatar name={displayName || 'You'} src={avatarSrc} size={72} />
          <button
            type="button"
            class="avatar-edit"
            aria-label={t('settings.account.changePhotoAriaLabel')}
            onclick={open}
          >
            <Upload size={13} />
          </button>
        </div>
      {/snippet}
    </AvatarPicker>

    <div class="profile-fields">
      <div class="field">
        <span class="field-label">{t('settings.account.displayNameLabel')}</span>
        <TextInput
          bind:value={displayName}
          placeholder={t('settings.account.displayNamePlaceholder')}
          width="260px"
          maxlength={DISPLAY_NAME_MAX}
        />
        {#if profileDirty && profileErrorText}
          <span class="field-error">{profileErrorText}</span>
        {/if}
      </div>
      {#if avatarSrc}
        <button
          type="button"
          class="remove-avatar"
          onclick={() => {
            avatarSrc = null;
            pendingAvatarBytes = null;
          }}
        >
          <Trash2 size={12} />
          {t('settings.account.removePhoto')}
        </button>
      {/if}
    </div>
  </div>

  {#snippet footer()}
    {#if profileDirty}
      <Button variant="secondary" onclick={resetProfile}>{t('settings.account.cancel')}</Button>
    {/if}
    <Button
      onclick={saveProfile}
      loading={savingProfile}
      success={profileSaved.active}
      disabled={!profileDirty || !!profileError}
    >
      <Save size={13} />
      {t('settings.account.save')}
    </Button>
  {/snippet}

  {#if profileState.userId}
    <div class="user-id-block">
      <div class="user-id-head">
        <span class="user-id-label">{t('settings.account.userIdLabel')}</span>
        <div class="user-id-actions">
          <button
            class="icon-btn"
            class:confirmed={copiedUserId.active}
            onclick={copyUserId}
            aria-label={copiedUserId.active
              ? t('common.copied')
              : t('settings.account.copyUserIdAriaLabel')}
          >
            {#if copiedUserId.active}
              <Check size={13} />
            {:else}
              <Copy size={13} />
            {/if}
          </button>
        </div>
      </div>
      <code class="user-id mono selectable">{profileState.userId}</code>
    </div>
  {/if}
</SettingCard>

<SettingCard
  title={t('settings.account.backupCard')}
  description={t('settings.account.backupDescription')}
>
  <SettingRow
    title={t('settings.account.exportRow')}
    description={t('settings.account.exportDescription')}
  >
    {#if exportOpen}
      <Button variant="secondary" onclick={resetExport}>{t('settings.account.cancel')}</Button>
    {:else}
      <Button variant="secondary" onclick={() => (exportOpen = true)} success={backupSaved.active}>
        <Download size={13} />
        {t('settings.account.exportButton')}
      </Button>
    {/if}
  </SettingRow>
  {#if exportOpen}
    <div class="pwd-block">
      <div class="field">
        <span class="field-label">{t('settings.account.passwordLabel')}</span>
        <TextInput
          type="password"
          bind:value={exportPwd}
          placeholder={t('settings.account.passwordPlaceholder')}
          width="260px"
        />
      </div>
      <div class="field">
        <span class="field-label">{t('settings.account.confirmLabel')}</span>
        <TextInput
          type="password"
          bind:value={exportPwd2}
          placeholder={t('settings.account.confirmPlaceholder')}
          width="260px"
        />
      </div>
      <p class="pwd-warn">
        {t('settings.account.passwordWarning')}
      </p>
      <div class="pwd-actions">
        <Button onclick={handleExportBackup} loading={exporting}>
          <Save size={13} />
          {t('settings.account.encryptAndSave')}
        </Button>
      </div>
    </div>
  {/if}

  <SettingRow
    title={t('settings.account.importRow')}
    description={t('settings.account.importDescription')}
  >
    {#if importOpen}
      <Button variant="secondary" onclick={resetImport}>{t('settings.account.cancel')}</Button>
    {:else}
      <Button variant="secondary" onclick={pickImportFile}>
        <FolderOpen size={13} />
        {t('settings.account.chooseFile')}
      </Button>
    {/if}
  </SettingRow>
  {#if importOpen}
    <div class="pwd-block">
      <div class="field">
        <span class="field-label">{t('settings.account.passwordLabel')}</span>
        <TextInput
          type="password"
          bind:value={importPwd}
          placeholder={t('settings.account.backupPasswordPlaceholder')}
          width="260px"
        />
      </div>
      <div class="pwd-actions">
        <Button onclick={handleImportBackup} loading={importing}>
          <Upload size={13} />
          {t('settings.account.decryptAndRestore')}
        </Button>
      </div>
    </div>
  {/if}
</SettingCard>

<SettingCard title={t('settings.account.notificationsCard')}>
  <SettingRow
    title={t('settings.account.systemPermissionRow')}
    description={notifPermission
      ? t('settings.account.permissionGranted')
      : t('settings.account.permissionDenied')}
  >
    <div class="notif-perm">
      <span class="perm-dot" data-on={notifPermission}></span>
      {#if notifPermission}
        <Button variant="secondary" onclick={handleTestNotification} loading={sendingTest}>
          <BellRing size={13} />
          {t('settings.account.sendTestButton')}
        </Button>
      {:else}
        <Button onclick={handleEnableNotifications} loading={requestingPerm}>
          <Bell size={13} />
          {t('settings.account.enableButton')}
        </Button>
      {/if}
    </div>
  </SettingRow>
  <SettingRow
    title={t('settings.account.messagePreviewRow')}
    description={t('settings.account.messagePreviewDescription')}
  >
    <Segmented
      value={preview}
      options={[
        { value: 'content', label: t('settings.account.previewFull') },
        { value: 'sender', label: t('settings.account.previewSender') },
        { value: 'generic', label: t('settings.account.previewGeneric') },
        { value: 'none', label: t('settings.account.previewOff') },
      ]}
      onchange={updatePreview}
      size="sm"
    />
  </SettingRow>
  <SettingRow
    title={t('settings.account.doNotDisturbRow')}
    description={t('settings.account.doNotDisturbDescription')}
  >
    <Toggle
      checked={dnd.enabled}
      ariaLabel={t('settings.account.doNotDisturbAriaLabel')}
      onchange={(v) => updateDnd({ enabled: v })}
    />
  </SettingRow>
  {#if dnd.enabled}
    <div class="schedule-block">
      <DndDial
        start={dnd.start}
        end={dnd.end}
        onchange={(s, e) => updateDnd({ start: s, end: e })}
      />
    </div>
  {/if}
</SettingCard>

<style>
  .icon-btn.confirmed {
    color: var(--success);
  }

  .user-id-block {
    padding: 14px;
    border-bottom: 1px solid var(--border-light);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .user-id-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }
  .user-id-label {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-primary);
  }
  .user-id-actions {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .user-id {
    font-family: var(--font-mono);
    font-size: var(--text-2xs);
    color: var(--text-secondary);
    background: var(--bg-input);
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    word-break: break-all;
    line-height: 1.5;
  }
  .profile {
    display: flex;
    gap: 20px;
    padding: 16px;
    align-items: flex-start;
  }
  .avatar-wrap {
    position: relative;
    flex-shrink: 0;
  }
  .avatar-edit {
    position: absolute;
    right: -2px;
    bottom: -2px;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    background: var(--accent);
    color: #fff;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 2px solid var(--bg-tertiary);
    cursor: pointer;
    transition: background var(--transition);
  }
  @media (hover: hover) {
    .avatar-edit:hover {
      background: var(--accent-hover);
    }
  }
  .profile-fields {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .field-label {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-secondary);
  }
  .field-error {
    font-size: var(--text-xs);
    color: var(--danger);
    max-width: 260px;
    line-height: 1.4;
  }
  .remove-avatar {
    align-self: flex-start;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: var(--text-xs);
    font-weight: 500;
    color: var(--danger);
    border: none;
    background: none;
    padding: 2px 0;
    cursor: pointer;
  }
  @media (hover: hover) {
    .remove-avatar:hover {
      opacity: 0.8;
    }
  }

  .schedule-block {
    padding: 14px 14px 18px;
    border-top: 1px solid var(--border-light);
  }
  .pwd-block {
    padding: 14px;
    border-top: 1px solid var(--border-light);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .pwd-warn {
    font-size: var(--text-xs);
    color: var(--text-muted);
    margin: 2px 0 0;
  }
  .pwd-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }
  .notif-perm {
    display: inline-flex;
    align-items: center;
    gap: 10px;
  }
  .perm-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--danger);
    box-shadow: 0 0 0 2px rgba(239, 68, 68, 0.18);
  }
  .perm-dot[data-on='true'] {
    background: var(--success, #22c55e);
    box-shadow: 0 0 0 2px rgba(34, 197, 94, 0.22);
  }
</style>
