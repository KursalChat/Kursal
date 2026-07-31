<script lang="ts">
  import { Share2, Search, X } from 'lucide-svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { trapFocus } from '$lib/utils/focusTrap';
  import Avatar from '$lib/components/Avatar.svelte';
  import { t } from '$lib/i18n';
  import type { SharePayload } from '$lib/types';

  let {
    payload,
    onPick,
    onCancel,
  }: {
    payload: SharePayload;
    onPick: (contactId: string) => void;
    onCancel: () => void;
  } = $props();

  let query = $state('');

  const filtered = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = contactsState.contacts;
    return q ? list.filter((c) => c.displayName.toLowerCase().includes(q)) : list;
  });

  const summary = $derived.by(() => {
    if (payload.files.length === 1) return payload.files[0].filename;
    if (payload.files.length > 1)
      return t('shareTarget.manyFiles', { count: payload.files.length });
    return payload.text ?? t('shareTarget.oneFile');
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault();
      e.stopPropagation();
      onCancel();
    }
  }
</script>

<div class="st-backdrop" onclick={onCancel} role="presentation"></div>
<div
  class="st-panel"
  role="dialog"
  aria-modal="true"
  aria-label={t('shareTarget.dialogAriaLabel')}
  tabindex="-1"
  onkeydown={onKeydown}
  use:trapFocus
>
  <div class="st-head">
    <h3><Share2 size={16} /> {t('shareTarget.title')}</h3>
    <button class="st-close" onclick={onCancel} aria-label={t('shareTarget.cancel')}>
      <X size={16} />
    </button>
  </div>

  <p class="st-preview">{summary}</p>

  <div class="st-search">
    <Search size={14} />
    <input
      type="text"
      placeholder={t('shareTarget.searchPlaceholder')}
      bind:value={query}
      spellcheck="false"
      autocorrect="off"
      autocapitalize="off"
    />
  </div>

  <div class="st-list">
    {#if contactsState.contacts.length === 0}
      <div class="st-empty">{t('shareTarget.emptyContacts')}</div>
    {:else if filtered.length === 0}
      <div class="st-empty">{t('shareTarget.noContacts')}</div>
    {:else}
      {#each filtered as c (c.userId)}
        <button class="st-row" onclick={() => onPick(c.userId)}>
          <Avatar name={c.displayName} src={c.avatarBase64} size={34} />
          <span class="st-name">{c.displayName}</span>
        </button>
      {/each}
    {/if}
  </div>
</div>

<style>
  .st-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    z-index: 1000;
    animation: st-fade 120ms ease;
  }
  .st-panel {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(420px, 92vw);
    max-height: min(80vh, 560px);
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
    z-index: 1001;
    overflow: hidden;
    animation: st-pop 160ms cubic-bezier(0.2, 0.9, 0.3, 1.1);
  }
  @keyframes st-fade {
    from {
      opacity: 0;
    }
  }
  @keyframes st-pop {
    from {
      opacity: 0;
      transform: translate(-50%, -48%) scale(0.97);
    }
  }
  .st-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px 10px;
    flex-shrink: 0;
  }
  .st-head h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0;
    font-size: 15px;
    font-weight: 700;
    color: var(--text-primary);
  }
  .st-head :global(svg) {
    color: var(--accent);
  }
  .st-close {
    width: 28px;
    height: 28px;
    border-radius: var(--radius-md);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    transition: all var(--transition);
  }
  .st-close:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .st-preview {
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
    word-break: break-word;
  }
  .st-search {
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
  .st-search:focus-within {
    border-color: var(--accent-selected);
  }
  .st-search input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text-primary);
    font-size: 13px;
  }
  .st-list {
    overflow-y: auto;
    padding: 4px 8px max(10px, var(--safe-bottom));
  }
  .st-empty {
    padding: 24px 12px;
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
  }
  .st-row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 8px 10px;
    border-radius: var(--radius-md);
    text-align: left;
    transition: background var(--transition);
  }
  .st-row:hover {
    background: var(--bg-hover);
  }
  .st-name {
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
