<script lang="ts">
  import { X, Search, Forward } from 'lucide-svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { messagesState } from '$lib/state/messages.svelte';
  import { sendText } from '$lib/api/messages';
  import { notifications } from '$lib/state/notifications.svelte';
  import { trapFocus } from '$lib/utils/focusTrap';
  import { log } from '$lib/utils/log';
  import Avatar from '$lib/components/Avatar.svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { t } from '$lib/i18n';

  let { content, onClose }: { content: string; onClose: () => void } = $props();

  let query = $state('');
  let sendingTo = $state<string | null>(null);

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = contactsState.contacts;
    return q ? list.filter((c) => c.displayName.toLowerCase().includes(q)) : list;
  });

  async function forward(userId: string, name: string) {
    if (sendingTo) return;
    sendingTo = userId;
    const pendingId = crypto.randomUUID().replace(/-/g, '');
    messagesState.appendOptimistic({
      id: pendingId,
      contactId: userId,
      direction: 'sent',
      content,
      status: 'sending',
      timestamp: Date.now(),
      receivedTimestamp: Date.now(),
      replyTo: null,
    });
    try {
      const realId = await sendText(userId, content);
      messagesState.replaceId(pendingId, userId, realId);
    } catch (e) {
      messagesState.updateStatusIfSending(pendingId, userId, 'queued');
      log.error('Forward send failed, queued for offline:', e);
    }
    notifications.push(t('chat.forward.success', { name }), 'success');
    onClose();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onClose();
    }
  }
</script>

<div class="fw-backdrop" onclick={onClose} role="presentation"></div>
<div
  class="fw-panel"
  role="dialog"
  aria-modal="true"
  aria-label={t('chat.forward.title')}
  tabindex="-1"
  onkeydown={onKeydown}
  use:trapFocus
>
  <div class="fw-head">
    <h3><Forward size={16} /> {t('chat.forward.title')}</h3>
    <button class="fw-close" onclick={onClose} aria-label={t('common.close')}>
      <X size={16} />
    </button>
  </div>

  <p class="fw-preview">{content}</p>

  <div class="fw-search">
    <Search size={14} />
    <input
      type="text"
      placeholder={t('chat.forward.searchPlaceholder')}
      bind:value={query}
      spellcheck="false"
      autocorrect="off"
      autocapitalize="off"
    />
  </div>

  <div class="fw-list">
    {#if filtered.length === 0}
      <div class="fw-empty">{t('chat.forward.noContacts')}</div>
    {:else}
      {#each filtered as c (c.userId)}
        <button
          class="fw-row"
          disabled={!!sendingTo}
          onclick={() => forward(c.userId, c.displayName)}
        >
          <Avatar name={c.displayName} src={c.avatarBase64} size={34} />
          <span class="fw-name">{c.displayName}</span>
          {#if sendingTo === c.userId}
            <Spinner size={14} />
          {/if}
        </button>
      {/each}
    {/if}
  </div>
</div>

<style>
  .fw-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    z-index: 1000;
    animation: fw-fade 120ms ease;
  }
  .fw-panel {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(420px, 92vw);
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
    z-index: 1001;
    overflow: hidden;
    animation: fw-pop 160ms cubic-bezier(0.2, 0.9, 0.3, 1.1);
  }
  @keyframes fw-fade {
    from {
      opacity: 0;
    }
  }
  @keyframes fw-pop {
    from {
      opacity: 0;
      transform: translate(-50%, -48%) scale(0.97);
    }
  }
  .fw-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px 10px;
    flex-shrink: 0;
  }
  .fw-head h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: 15px;
    font-weight: 700;
    color: var(--text-primary);
  }
  .fw-head :global(svg) {
    color: var(--accent);
  }
  .fw-close {
    width: 28px;
    height: 28px;
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    transition: all var(--transition);
  }
  .fw-close:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .fw-preview {
    margin: 0 16px 10px;
    padding: 8px 10px;
    background: var(--bg-tertiary);
    border-radius: var(--radius-md);
    font-size: 12.5px;
    color: var(--text-secondary);
    max-height: 60px;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
  }
  .fw-search {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 16px 8px;
    padding: 7px 10px;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text-muted);
  }
  .fw-search:focus-within {
    border-color: var(--accent-selected);
  }
  .fw-search input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 13px;
  }
  .fw-list {
    overflow-y: auto;
    padding: 4px 8px 10px;
  }
  .fw-empty {
    padding: 24px 12px;
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
  }
  .fw-row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    text-align: left;
    transition: background var(--transition);
  }
  .fw-row:hover:not(:disabled) {
    background: var(--bg-hover);
  }
  .fw-row:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
  .fw-name {
    flex: 1;
    min-width: 0;
    font-size: 14px;
    font-weight: 500;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
