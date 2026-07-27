<script lang="ts">
  import { scale } from 'svelte/transition';
  import { log } from '$lib/utils/log';
  import { getSecurityCode, confirmSecurityCode } from '$lib/api/contacts';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { errorText } from '$lib/utils/errors';
  import { trapFocus } from '$lib/utils/focusTrap';
  import Button from './Button.svelte';
  import { t } from '$lib/i18n';

  let {
    contactId,
    onClose,
    contactVerified = false,
  }: {
    contactId: string;
    onClose: () => void;
    contactVerified?: boolean;
  } = $props();

  let code = $state<string | null>(null);
  let loading = $state(false);
  let confirming = $state(false);
  let error = $state<string | null>(null);

  async function loadCode() {
    loading = true;
    try {
      code = await getSecurityCode(contactId);
      error = null;
    } catch (e) {
      error = errorText(e);
      log.error('Failed to load security code:', e);
    } finally {
      loading = false;
    }
  }

  async function handleConfirm() {
    confirming = true;
    try {
      await confirmSecurityCode(contactId);
      contactsState.markVerified(contactId);
      notifications.push(t('securityCode.successVerified'), 'success');
      onClose();
    } catch (e) {
      error = errorText(e);
      log.error('Failed to confirm security code:', e);
    } finally {
      confirming = false;
    }
  }

  async function copyCode() {
    if (code) {
      try {
        await navigator.clipboard.writeText(code);
        notifications.push(t('securityCode.copied'), 'success');
      } catch (e) {
        log.error('Failed to copy code:', e);
      }
    }
  }

  import { onMount } from 'svelte';
  onMount(() => {
    loadCode();
  });

  function handleBackdropClick(e: MouseEvent) {
    if (e.target === e.currentTarget) {
      onClose();
    }
  }
</script>

<div
  class="backdrop"
  role="presentation"
  onclick={handleBackdropClick}
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
    aria-label={t('securityCode.heading')}
    tabindex="-1"
    use:trapFocus
  >
    <h2>{t('securityCode.heading')}</h2>

    {#if error}
      <div class="error">{error}</div>
    {/if}

    {#if loading}
      <div class="loading">{t('securityCode.loading')}</div>
    {:else if code}
      <p class="explanation">
        {t('securityCode.explanation')}
      </p>

      <ol class="code-grid">
        {#each code.split(/\s+/).filter((s) => s.length > 0) as word, i}
          <li class="code-cell">
            <span class="cell-index">{(i + 1).toString().padStart(2, '0')}</span>
            <span class="cell-value">{word}</span>
          </li>
        {/each}
      </ol>

      <Button variant="secondary" onclick={copyCode}>
        {t('securityCode.copyButton')}
      </Button>

      {#if contactVerified}
        <Button variant="primary" onclick={onClose}>{t('securityCode.closeButton')}</Button>
      {:else}
        <Button variant="primary" loading={confirming} onclick={handleConfirm}>
          {t('securityCode.confirmButton')}
        </Button>
        <button class="link" onclick={onClose}>{t('securityCode.doLaterButton')}</button>
      {/if}
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    border: none;
    animation: backdrop-in 0.2s ease;
    cursor: default;
    padding: 0;
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
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: var(--shadow-3);
    padding: 32px;
    max-width: 400px;
    width: 90%;
    text-align: center;
  }

  h2 {
    margin-bottom: 16px;
    font-size: var(--text-lg);
  }

  .error {
    background: var(--danger-dim);
    color: var(--danger);
    padding: 12px;
    border-radius: var(--radius-md);
    margin-bottom: 16px;
    font-size: var(--text-sm);
  }

  .loading {
    padding: 24px;
    color: var(--text-secondary);
  }

  .explanation {
    margin-bottom: 24px;
    font-size: 14px;
    color: var(--text-secondary);
    line-height: 1.6;
  }

  .code-grid {
    list-style: none;
    margin: 0 0 24px;
    padding: 0;
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }

  .code-cell {
    display: flex;
    align-items: baseline;
    gap: 10px;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 10px 12px;
    text-align: left;
  }

  .cell-index {
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
    padding-right: 10px;
    border-right: 1px solid var(--border);
  }

  .cell-value {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    font-weight: 600;
    overflow-wrap: anywhere;
    word-break: normal;
    color: var(--text-primary);
  }

  :global(.modal .button) {
    width: 100%;
    margin-bottom: 12px;
  }

  .link {
    display: block;
    width: 100%;
    text-align: center;
    color: var(--accent);
    font-size: 14px;
    cursor: pointer;
    background: none;
    border: none;
    padding: 8px 0;
  }

  .link:hover {
    color: var(--accent-hover);
  }
</style>
