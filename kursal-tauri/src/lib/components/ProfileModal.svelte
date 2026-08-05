<script lang="ts">
  import { scale } from 'svelte/transition';
  import { log } from '$lib/utils/log';
  import { X, Trash2, Ban, Shield, Copy, Pencil, Check } from 'lucide-svelte';
  import Avatar from './Avatar.svelte';
  import SecurityCodeModal from './SecurityCodeModal.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import type { ContactResponse } from '$lib/types';
  import { removeContact, setContactBlocked } from '$lib/api/contacts';
  import { confirmDialog } from '$lib/state/confirm.svelte';
  import { busy } from '$lib/utils/busy.svelte';
  import { flash } from '$lib/utils/flash.svelte';
  import { trapFocus } from '$lib/utils/focusTrap';
  import Spinner from './Spinner.svelte';
  import { t } from '$lib/i18n';

  const blockBusy = busy();
  const removeBusy = busy();
  const copiedUserId = flash();

  let { contact, onClose }: { contact: ContactResponse | null; onClose: () => void } = $props();

  let showSecurityModal = $state(false);
  let editingName = $state(false);
  let nameInput = $state('');

  function startEditName() {
    if (!contact) return;
    nameInput = contactsState.aliasFor(contact.userId) ?? '';
    editingName = true;
  }

  async function saveAlias() {
    if (!contact) return;
    try {
      await contactsState.setAlias(contact.userId, nameInput);
      editingName = false;
    } catch (e) {
      notifications.push(t('profile.errorNickname'), 'error');
      log.error(e);
    }
  }

  async function copyUserId() {
    if (!contact) return;
    try {
      await navigator.clipboard.writeText(contact.userId);
      copiedUserId.trigger();
    } catch (e) {
      log.error('Copy failed', e);
    }
  }

  async function handleToggleBlock() {
    if (!contact) return;
    const userId = contact.userId;
    const displayName = contact.displayName;
    const willBlock = !contact.blocked;

    const confirmed = await confirmDialog({
      title: t(willBlock ? 'profile.blockConfirmTitle' : 'profile.unblockConfirmTitle', {
        name: displayName,
      }),
      message: t(willBlock ? 'profile.blockConfirmMessage' : 'profile.unblockConfirmMessage'),
      tone: 'warning',
      confirmLabel: t(willBlock ? 'profile.blockConfirmLabel' : 'profile.unblockConfirmLabel'),
    });

    if (!confirmed) return;

    await blockBusy.run(async () => {
      try {
        await setContactBlocked(userId, willBlock);
        contactsState.upsert({ ...contact, blocked: willBlock });
        notifications.push(
          t(willBlock ? 'profile.successBlocked' : 'profile.successUnblocked', {
            name: displayName,
          }),
          'success'
        );
      } catch (e) {
        notifications.push(t(willBlock ? 'profile.errorBlock' : 'profile.errorUnblock'), 'error');
        log.error(e);
      }
    });
  }

  async function handleRemoveContact() {
    if (!contact) return;

    const userId = contact.userId;
    const displayName = contact.displayName;

    const confirmed = await confirmDialog({
      title: t('profile.removeConfirmTitle', { name: displayName }),
      message: t('profile.removeConfirmMessage'),
      tone: 'danger',
      confirmLabel: t('profile.removeConfirmLabel'),
    });

    if (!confirmed) return;

    await removeBusy.run(async () => {
      try {
        await removeContact(userId);
        notifications.push(t('profile.successRemoved', { name: displayName }), 'success');
        onClose();
      } catch (e) {
        notifications.push(t('profile.errorRemove'), 'error');
        log.error(e);
      }
    });
  }

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }
</script>

<div
  class="modal-backdrop"
  role="presentation"
  onclick={handleBackdropClick}
  onkeydown={(e) => {
    if (e.key === 'Escape') onClose();
  }}
>
  <div
    class="modal"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    use:trapFocus={{ active: !showSecurityModal }}
    in:scale={{ duration: 200, start: 0.94, opacity: 0 }}
    out:scale={{ duration: 150, start: 0.94, opacity: 0 }}
  >
    <button class="close-btn" onclick={onClose} aria-label={t('common.close')}>
      <X size={20} />
    </button>

    {#if contact}
      <div class="profile-content">
        <Avatar name={contact.displayName} src={contact.avatarBase64} size={84} />
        {#if editingName}
          <div class="name-row">
            <input
              class="nickname-input"
              bind:value={nameInput}
              maxlength="32"
              placeholder={contact.profileName ?? contact.displayName}
              onkeydown={(e) => {
                e.stopPropagation();
                if (e.key === 'Enter') void saveAlias();
                if (e.key === 'Escape') editingName = false;
              }}
            />
            <button class="icon-btn" onclick={saveAlias} aria-label={t('common.save')}>
              <Check size={15} />
            </button>
          </div>
        {:else}
          <div class="name-row">
            <h2>{contact.displayName}</h2>
            <button class="icon-btn" onclick={startEditName} aria-label={t('profile.nicknameEdit')}>
              <Pencil size={13} />
            </button>
          </div>
          {#if contactsState.aliasFor(contact.userId) && contact.profileName}
            <span class="real-name">~ {contact.profileName}</span>
          {/if}
        {/if}

        <div class="user-id-card">
          <div class="user-id-row">
            <span class="user-id-label">{t('profile.userIdLabel')}</span>
            <button
              class="copy-btn"
              class:confirmed={copiedUserId.active}
              onclick={copyUserId}
              title={t('profile.copyUserId')}
              aria-label={copiedUserId.active ? t('common.copied') : t('profile.copyUserId')}
            >
              {#if copiedUserId.active}
                <Check size={13} />
              {:else}
                <Copy size={13} />
              {/if}
            </button>
          </div>
          <code class="user-id-value">{contact.userId}</code>
        </div>
      </div>

      <div class="actions">
        <button class="secondary-btn" onclick={() => (showSecurityModal = true)}>
          <Shield size={16} />
          {contact.verified ? t('profile.viewSecurityCode') : t('profile.verifySecurityCode')}
        </button>
        <button
          class="danger-btn block-btn"
          onclick={handleToggleBlock}
          disabled={blockBusy.active}
        >
          {#if blockBusy.active}
            <Spinner size={16} color="currentColor" />
          {:else}
            <Ban size={16} />
          {/if}
          {contact.blocked ? t('profile.unblockContact') : t('profile.blockContact')}
        </button>
        <button class="danger-btn" onclick={handleRemoveContact} disabled={removeBusy.active}>
          {#if removeBusy.active}
            <Spinner size={16} color="currentColor" />
          {:else}
            <Trash2 size={16} />
          {/if}
          {t('profile.removeContact')}
        </button>
      </div>
    {/if}
  </div>
</div>

{#if showSecurityModal && contact}
  <SecurityCodeModal
    contactId={contact.userId}
    contactVerified={contact.verified}
    onClose={() => (showSecurityModal = false)}
  />
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(2, 6, 23, 0.7);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: var(--safe-top) var(--safe-right) var(--safe-bottom) var(--safe-left);
    z-index: 100;
    animation: backdrop-in 0.18s ease;
  }
  @keyframes backdrop-in {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .modal {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    width: 90%;
    max-width: 320px;
    max-height: 100%;
    overflow-y: auto;
    padding: 32px 24px 24px;
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 32px;
    box-shadow: var(--glow);
  }

  .close-btn {
    position: absolute;
    top: 16px;
    right: 16px;
    color: var(--text-muted);
    transition: color var(--transition);
  }

  .close-btn:hover {
    color: var(--text-primary);
  }

  .profile-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    text-align: center;
  }

  .profile-content h2 {
    font-size: 20px;
    margin: 0;
  }

  .name-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    color: var(--text-muted);
    transition: all var(--transition);
  }
  .icon-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .nickname-input {
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 6px 10px;
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary);
    width: 180px;
    text-align: center;
  }
  .real-name {
    font-size: 12px;
    color: var(--text-muted);
    margin-top: -8px;
  }

  .user-id-card {
    width: 100%;
    border: 1px solid var(--border);
    background: var(--bg-tertiary);
    border-radius: var(--radius-md);
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    text-align: left;
  }

  .user-id-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .user-id-label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-muted);
  }

  .copy-btn {
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    border-radius: 6px;
    transition: all var(--transition);
  }

  .copy-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }

  .copy-btn.confirmed,
  .copy-btn.confirmed:hover {
    color: var(--success);
  }

  .user-id-value {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text-secondary);
    word-break: break-all;
    line-height: 1.4;
  }

  .secondary-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    padding: 12px;
    border-radius: var(--radius-md);
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    color: var(--text-primary);
    font-weight: 600;
    transition: background var(--transition);
  }

  .secondary-btn:hover {
    background: var(--bg-hover);
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .danger-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    width: 100%;
    padding: 12px;
    border-radius: var(--radius-md);
    background: rgba(251, 113, 133, 0.1);
    color: var(--danger);
    font-weight: 600;
    transition: background var(--transition);
  }

  .danger-btn:hover:not(:disabled) {
    background: rgba(251, 113, 133, 0.2);
  }

  .danger-btn:disabled,
  .secondary-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
