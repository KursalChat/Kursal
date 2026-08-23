<script lang="ts">
  import { onMount } from 'svelte';
  import { log } from '$lib/utils/log';
  import { getSecurityCode, confirmSecurityCode } from '$lib/api/contacts';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { notifications } from '$lib/state/notifications.svelte';
  import { errorText } from '$lib/utils/errors';
  import { flash } from '$lib/utils/flash.svelte';
  import { copyText } from '$lib/utils/clipboard';
  import Modal from './Modal.svelte';
  import Button from './Button.svelte';
  import { t } from '$lib/i18n';

  const copied = flash();

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
  const codeWords = $derived(code ? code.split(/\s+/).filter((s) => s.length > 0) : []);
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
    if (code) await copyText(code, { flash: copied });
  }

  onMount(() => {
    loadCode();
  });
</script>

<Modal title={t('securityCode.heading')} {onClose} width={400} padding="32px">
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
      {#each codeWords as word, i (word + i)}
        <li class="code-cell">
          <span class="cell-index">{(i + 1).toString().padStart(2, '0')}</span>
          <span class="cell-value">{word}</span>
        </li>
      {/each}
    </ol>

    <Button
      variant="secondary"
      onclick={copyCode}
      success={copied.active}
      successLabel={t('common.copied')}
    >
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
</Modal>

<style>
  h2,
  .explanation,
  .loading,
  .error {
    text-align: center;
  }

  h2 {
    margin: 0;
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

  @media (hover: hover) {
    .link:hover {
      color: var(--accent-hover);
    }
  }
</style>
