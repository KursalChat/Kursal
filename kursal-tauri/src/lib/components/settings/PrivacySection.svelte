<script lang="ts">
  import { onMount } from 'svelte';
  import { log } from '$lib/utils/log';
  import { RefreshCw, ShieldOff, Trash2, Copy, TriangleAlert, Fingerprint } from 'lucide-svelte';
  import { checkStatus } from '@tauri-apps/plugin-biometric';
  import { isMobile } from '$lib/api/window';
  import { prefsState } from '$lib/state/prefs.svelte';
  import { writeText } from '@tauri-apps/plugin-clipboard-manager';
  import { rotatePeerId } from '$lib/api/identity';
  import {
    listBlockedContacts,
    clearMessageHistory,
    deleteAllLocalData,
    type PeerRotationInterval,
  } from '$lib/api/settings';
  import { settingsState } from '$lib/state/settings.svelte';
  import { confirmDialog } from '$lib/state/confirm.svelte';
  import type { ContactResponse } from '$lib/types';
  import { profileState } from '$lib/state/profile.svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { notifyError } from '$lib/utils/errors';
  import { messagesState } from '$lib/state/messages.svelte';
  import Avatar from '$lib/components/Avatar.svelte';
  import Button from '$lib/components/Button.svelte';
  import SettingCard from './SettingCard.svelte';
  import SettingRow from './SettingRow.svelte';
  import Toggle from './Toggle.svelte';
  import Segmented from './Segmented.svelte';
  import Select from './Select.svelte';
  import { setContactBlocked } from '$lib/api/contacts';
  import { t } from '$lib/i18n';

  let rotating = $state(false);
  const rotationInterval = $derived(settingsState.peerRotation);
  const typing = $derived(settingsState.typingIndicators);

  let blocked = $state<ContactResponse[]>([]);
  let blockedLoading = $state(false);

  let clearTarget = $state<string>('');
  let clearing = $state(false);
  let deleting = $state(false);

  let biometricAvailable = $state(false);
  const appLockEnabled = $derived(prefsState.appLockBiometric);

  onMount(async () => {
    void settingsState.load();
    prefsState.init();
    await reloadBlocked();
    if (isMobile) {
      try {
        const status = await checkStatus();
        biometricAvailable = status.isAvailable;
      } catch (e) {
        log.error('biometric status check failed', e);
        biometricAvailable = false;
      }
    }
  });

  async function handleAppLockChange(value: boolean) {
    if (value && isMobile) {
      try {
        const { authenticate } = await import('@tauri-apps/plugin-biometric');
        await authenticate(t('biometricLock.enableReason'), {
          allowDeviceCredential: true,
          cancelTitle: t('biometricLock.cancelTitle'),
          fallbackTitle: t('biometricLock.fallbackTitle'),
          title: t('biometricLock.androidTitle'),
          subtitle: t('biometricLock.androidSubtitle'),
          confirmationRequired: false,
        });
      } catch (e) {
        notifications.push(t('settings.privacy.errorAppLockEnable'), 'error');
        return;
      }
    }
    prefsState.setAppLockBiometric(value);
    notifications.push(
      value
        ? t('settings.privacy.successAppLockEnabled')
        : t('settings.privacy.successAppLockDisabled'),
      'success'
    );
  }

  async function reloadBlocked() {
    blockedLoading = true;
    try {
      blocked = await listBlockedContacts();
    } catch (e) {
      log.error(e);
    } finally {
      blockedLoading = false;
    }
  }

  async function handleRotate() {
    const ok = await confirmDialog({
      title: t('settings.privacy.rotatePeerIdTitle'),
      message: t('settings.privacy.rotatePeerIdMessage'),
      detail: t('settings.privacy.rotatePeerIdDetail'),
      confirmLabel: t('settings.privacy.rotatePeerIdConfirm'),
      tone: 'warning',
    });
    if (!ok) return;
    rotating = true;
    try {
      await rotatePeerId();
      await profileState.refreshPeerId();
      notifications.push(t('settings.privacy.successPeerIdRotated'), 'success');
    } catch (e) {
      notifications.push(t('settings.privacy.errorPeerIdRotate'), 'error');
    } finally {
      rotating = false;
    }
  }

  async function handleIntervalChange(value: PeerRotationInterval) {
    try {
      await settingsState.setPeerRotation(value);
      notifications.push(t('settings.privacy.infoRestartRequired'), 'info');
    } catch (e) {
      notifyError(e);
    }
  }

  async function copyPeerId() {
    if (!profileState.peerId) return;
    try {
      await writeText(profileState.peerId);
      notifications.push(t('settings.privacy.successPeerIdCopied'), 'success');
    } catch (e) {
      notifyError(e, 'settings.privacy.errorCopyFailed');
    }
  }

  async function handleTypingChange(value: boolean) {
    try {
      await settingsState.setTyping(value);
    } catch (e) {
      notifyError(e);
    }
  }

  async function handleReadReceiptsChange(value: boolean) {
    try {
      await settingsState.setReadReceipts(value);
    } catch (e) {
      notifyError(e);
    }
  }

  async function handleUnblock(id: string) {
    try {
      await setContactBlocked(id, false);
      blocked = blocked.filter((c) => c.userId !== id);
      notifications.push(t('settings.privacy.successContactUnblocked'), 'success');
    } catch (e) {
      notifyError(e);
    }
  }

  async function handleClearHistory() {
    if (!clearTarget) return;
    const targetLabel =
      clearTarget === '*'
        ? t('settings.privacy.allContacts')
        : (contactsState.getById(clearTarget)?.displayName ?? t('settings.privacy.thisContact'));
    const ok = await confirmDialog({
      title: t('settings.privacy.clearHistoryDialogTitle'),
      message: t('settings.privacy.clearHistoryDialogMessage', { target: targetLabel }),
      detail: t('settings.privacy.clearHistoryDialogDetail'),
      confirmLabel: t('settings.privacy.clearHistoryDialogConfirm'),
      tone: 'danger',
      holdMs: 5000,
    });
    if (!ok) return;
    clearing = true;
    try {
      await clearMessageHistory(clearTarget === '*' ? null : clearTarget);
      if (clearTarget === '*') {
        messagesState.clearAll();
      } else {
        messagesState.clearForContact(clearTarget);
      }
      notifications.push(t('settings.privacy.successHistoryCleared'), 'success');
    } catch (e) {
      notifyError(e);
    } finally {
      clearing = false;
    }
  }

  async function handleDeleteAll() {
    const first = await confirmDialog({
      title: t('settings.privacy.deleteAllDialog1Title'),
      message: t('settings.privacy.deleteAllDialog1Message'),
      detail: t('settings.privacy.deleteAllDialog1Detail'),
      confirmLabel: t('settings.privacy.deleteAllDialog1Confirm'),
      tone: 'danger',
    });
    if (!first) return;
    const second = await confirmDialog({
      title: t('settings.privacy.deleteAllDialog2Title'),
      message: t('settings.privacy.deleteAllDialog2Message'),
      detail: t('settings.privacy.deleteAllDialog2Title'),
      confirmLabel: t('settings.privacy.deleteAllDialog2Confirm'),
      cancelLabel: t('settings.privacy.deleteAllDialog2Cancel'),
      tone: 'danger',
      holdMs: 5000,
    });
    if (!second) return;
    deleting = true;
    try {
      await deleteAllLocalData();
    } catch (e) {
      notifyError(e);
      deleting = false;
    }
  }

  const intervals: { value: PeerRotationInterval; label: string }[] = [
    { value: '6h', label: '6h' },
    { value: '12h', label: '12h' },
    { value: '30h', label: '30h' },
    { value: '7d', label: '7d' },
    { value: 'manual', label: 'Manual' },
  ];

  const clearOptions = $derived([
    { value: '*', label: t('settings.privacy.allContacts') },
    ...contactsState.contacts.map((c) => ({
      value: c.userId,
      label: c.displayName,
    })),
  ]);
</script>

<div class="sec-head">
  <h2>{t('settings.privacy.heading')}</h2>
  <p>{t('settings.privacy.description')}</p>
</div>

<SettingCard title={t('settings.privacy.identityCard')}>
  {#if profileState.peerId}
    <div class="peer-id-block">
      <div class="peer-id-head">
        <span class="peer-id-label">{t('settings.privacy.peerIdLabel')}</span>
        <div class="peer-id-actions">
          <button
            class="icon-btn"
            onclick={copyPeerId}
            aria-label={t('settings.privacy.copyPeerIdAriaLabel')}
          >
            <Copy size={13} />
          </button>
          <Button variant="secondary" loading={rotating} onclick={handleRotate}>
            <RefreshCw size={13} />
            {t('settings.privacy.rotateButton')}
          </Button>
        </div>
      </div>
      <code class="peer-id mono selectable">{profileState.peerId}</code>
    </div>
  {/if}
  <SettingRow
    title={t('settings.privacy.autoRotationRow')}
    description={t('settings.privacy.autoRotationDescription')}
  >
    <Segmented
      value={rotationInterval}
      options={intervals}
      onchange={handleIntervalChange}
      size="sm"
    />
  </SettingRow>
</SettingCard>

{#if isMobile && biometricAvailable}
  <SettingCard title={t('settings.privacy.appLockCard')}>
    <SettingRow
      title={t('settings.privacy.appLockBiometricRow')}
      description={t('settings.privacy.appLockBiometricDescription')}
    >
      <div class="biometric-row">
        <Fingerprint size={16} />
        <Toggle
          checked={appLockEnabled}
          onchange={handleAppLockChange}
          ariaLabel={t('settings.privacy.appLockBiometricAriaLabel')}
        />
      </div>
    </SettingRow>
  </SettingCard>
{/if}

<SettingCard title={t('settings.privacy.messagingCard')}>
  <SettingRow
    title={t('settings.privacy.typingIndicatorsRow')}
    description={t('settings.privacy.typingIndicatorsDescription')}
  >
    <Toggle
      checked={typing}
      onchange={handleTypingChange}
      ariaLabel={t('settings.privacy.typingIndicatorsAriaLabel')}
    />
  </SettingRow>
  <SettingRow
    title={t('settings.privacy.readReceiptsRow')}
    description={t('settings.privacy.readReceiptsDescription')}
  >
    <Toggle
      checked={settingsState.readReceipts}
      onchange={handleReadReceiptsChange}
      ariaLabel={t('settings.privacy.readReceiptsAriaLabel')}
    />
  </SettingRow>
</SettingCard>

<SettingCard
  title={t('settings.privacy.blockedCard')}
  description={t('settings.privacy.blockedDescription')}
>
  {#if blockedLoading}
    <div class="empty">{t('settings.privacy.blockedLoading')}</div>
  {:else if blocked.length === 0}
    <div class="empty">{t('settings.privacy.noBlocked')}</div>
  {:else}
    {#each blocked as c}
      <SettingRow title={c.displayName} description={c.userId.slice(0, 24) + '…'}>
        <Avatar name={c.displayName} src={c.avatarBase64} size={28} />
        <Button variant="secondary" onclick={() => handleUnblock(c.userId)}>
          <ShieldOff size={13} />
          {t('settings.privacy.unblockButton')}
        </Button>
      </SettingRow>
    {/each}
  {/if}
</SettingCard>

<SettingCard
  title={t('settings.privacy.clearHistoryCard')}
  description={t('settings.privacy.clearHistoryCardDescription')}
>
  <SettingRow
    title={t('settings.privacy.clearTargetRow')}
    description={t('settings.privacy.clearTargetDescription')}
  >
    <Select
      value={clearTarget}
      options={clearOptions}
      onchange={(v) => (clearTarget = v)}
      placeholder={t('settings.privacy.clearContactPlaceholder')}
      minWidth="220px"
    />
    <Button
      variant="danger"
      disabled={!clearTarget}
      loading={clearing}
      onclick={handleClearHistory}
    >
      <Trash2 size={13} />
      {t('settings.privacy.clearButton')}
    </Button>
  </SettingRow>
</SettingCard>

<SettingCard title={t('settings.privacy.dangerCard')} tone="danger">
  <div class="danger-body">
    <div class="danger-head">
      <TriangleAlert size={16} />
      <div>
        <div class="danger-title">{t('settings.privacy.deleteAllDataTitle')}</div>
        <div class="danger-desc">
          {t('settings.privacy.deleteAllDataDescription')}
        </div>
      </div>
    </div>
    <Button variant="danger" loading={deleting} onclick={handleDeleteAll}>
      <Trash2 size={13} />
      {t('settings.privacy.deleteAllButton')}
    </Button>
  </div>
</SettingCard>

<style>
  .peer-id-block {
    padding: 14px;
    border-bottom: 1px solid var(--border-light);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .peer-id-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }
  .peer-id-label {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-primary);
  }
  .peer-id-actions {
    display: flex;
    gap: 6px;
    align-items: center;
  }
  .peer-id {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    background: var(--bg-input);
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    word-break: break-all;
    line-height: 1.5;
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

  .empty {
    padding: 20px 14px;
    text-align: center;
    font-size: 13px;
    color: var(--text-muted);
  }

  .danger-body {
    padding: 14px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    align-items: flex-start;
  }
  .danger-head {
    display: flex;
    gap: 10px;
    color: #fda4af;
    align-items: flex-start;
  }
  .danger-head > :global(svg) {
    flex-shrink: 0;
    margin-top: 2px;
  }
  .danger-title {
    font-size: 14px;
    font-weight: 700;
    margin-bottom: 2px;
  }
  .danger-desc {
    font-size: 12px;
    color: var(--text-secondary);
    line-height: 1.5;
  }
  .biometric-row {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    color: var(--text-secondary);
  }
</style>
