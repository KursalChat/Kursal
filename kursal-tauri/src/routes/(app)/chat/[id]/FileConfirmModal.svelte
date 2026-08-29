<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { FileText, X } from 'lucide-svelte';
  import Spinner from '$lib/components/Spinner.svelte';
  import { formatFileSize } from '$lib/utils/bytes';
  import { t } from '$lib/i18n';

  interface PendingFile {
    backendPath: string;
    filename: string;
    sizeBytes: number;
  }

  interface Props {
    files: PendingFile[];
    sending: boolean;
    maxLength: number;
    initialCaption?: string;
    onConfirm: (caption: string) => void;
    onCancel: () => void;
    onRemove: (backendPath: string) => void;
  }

  let {
    files,
    sending,
    maxLength,
    initialCaption = '',
    onConfirm,
    onCancel,
    onRemove,
  }: Props = $props();

  const totalBytes = $derived(files.reduce((sum, f) => sum + f.sizeBytes, 0));

  let dialogEl = $state<HTMLElement | null>(null);
  let captionEl = $state<HTMLTextAreaElement | null>(null);
  // Seeded once: the modal remounts per batch, so later prop changes are irrelevant.
  let caption = $state(untrack(() => initialCaption));

  const CAPTION_MAX_H = 96;

  // scrollHeight excludes borders while border-box height includes them, so the
  // border width has to be added back or the field scrolls one line too early.
  function autogrow() {
    if (!captionEl) return;
    captionEl.style.height = 'auto';
    const borders = captionEl.offsetHeight - captionEl.clientHeight;
    const full = captionEl.scrollHeight + borders;
    captionEl.style.height = `${Math.min(full, CAPTION_MAX_H)}px`;
    captionEl.style.overflowY = full > CAPTION_MAX_H ? 'auto' : 'hidden';
  }

  onMount(() => {
    // Pull focus off the composer so Enter reaches the modal, not the textarea.
    (captionEl ?? dialogEl)?.focus();
    autogrow();
    // Capture phase so the shortcut fires even though the dialog stops keydown
    // from bubbling. A focused button keeps its native Enter/Space activation.
    function onKey(e: KeyboardEvent) {
      if (sending) return;
      if (e.key === 'Enter') {
        if (document.activeElement instanceof HTMLButtonElement) return;
        // Shift+Enter writes a newline in the caption instead of sending.
        if (e.shiftKey && document.activeElement === captionEl) return;
        e.preventDefault();
        onConfirm(caption);
      } else if (e.key === 'Escape') {
        onCancel();
      }
    }
    window.addEventListener('keydown', onKey, true);
    return () => window.removeEventListener('keydown', onKey, true);
  });
</script>

<div
  class="file-confirm-backdrop"
  role="presentation"
  onclick={() => {
    if (!sending) onCancel();
  }}
  onkeydown={(e) => {
    if (e.key === 'Escape' && !sending) onCancel();
  }}
>
  <div
    class="file-confirm"
    role="dialog"
    aria-modal="true"
    aria-label={t('chat.fileConfirm.dialogAriaLabel')}
    bind:this={dialogEl}
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
    tabindex="-1"
  >
    <div class="file-confirm-icon"><FileText size={28} /></div>
    <h3>
      {files.length === 1
        ? t('chat.fileConfirm.heading')
        : t('chat.fileConfirm.headingMulti', { count: files.length })}
    </h3>
    <div class="file-confirm-list" class:single={files.length === 1}>
      {#each files as file (file.backendPath)}
        <div class="file-row">
          <span class="f-name" title={file.filename}>{file.filename}</span>
          {#if file.sizeBytes > 0}
            <span class="f-size">{formatFileSize(file.sizeBytes)}</span>
          {/if}
          {#if files.length > 1}
            <button
              class="f-remove"
              disabled={sending}
              onclick={() => onRemove(file.backendPath)}
              aria-label={t('chat.fileConfirm.removeAriaLabel', { name: file.filename })}
            >
              <X size={13} />
            </button>
          {/if}
        </div>
      {/each}
    </div>
    {#if files.length > 1 && totalBytes > 0}
      <span class="f-total">
        {t('chat.fileConfirm.totalSize', { size: formatFileSize(totalBytes) })}
      </span>
    {/if}
    <textarea
      class="fc-caption"
      bind:this={captionEl}
      bind:value={caption}
      oninput={autogrow}
      rows="1"
      maxlength={maxLength}
      disabled={sending}
      placeholder={t('chat.fileConfirm.captionPlaceholder')}
      aria-label={t('chat.fileConfirm.captionAriaLabel')}></textarea>
    <div class="file-confirm-actions">
      <button class="fc-btn ghost" onclick={onCancel} disabled={sending}>
        {t('chat.fileConfirm.cancel')}
      </button>
      <button class="fc-btn primary" onclick={() => onConfirm(caption)} disabled={sending}>
        {#if sending}
          <Spinner size={14} color="#fff" />
        {:else if files.length === 1}
          {t('chat.fileConfirm.send')}
        {:else}
          {t('chat.fileConfirm.sendMulti', { count: files.length })}
        {/if}
      </button>
    </div>
  </div>
</div>

<style>
  .file-confirm-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(2, 6, 23, 0.65);
    backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: max(16px, var(--safe-top)) max(16px, var(--safe-right)) max(16px, var(--safe-bottom))
      max(16px, var(--safe-left));
    z-index: 400;
    animation: fadeIn 0.14s ease;
  }
  .file-confirm {
    width: min(380px, 100%);
    max-height: 100%;
    overflow-y: auto;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 22px 22px 18px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    box-shadow: var(--glow);
    animation: modal-pop 0.2s cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  @keyframes modal-pop {
    from {
      opacity: 0;
      transform: scale(0.92) translateY(8px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }
  .file-confirm h3 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
  }
  .file-confirm-icon {
    width: 52px;
    height: 52px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: var(--accent-dim);
    color: var(--accent);
  }
  .file-confirm-list {
    width: 100%;
    display: flex;
    flex-direction: column;
    max-height: 220px;
    overflow-y: auto;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    scrollbar-width: thin;
  }
  .file-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 12px;
    border-bottom: 1px solid var(--border-light);
    min-width: 0;
  }
  .file-row:last-child {
    border-bottom: none;
  }
  .file-confirm-list.single .file-row {
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 10px 12px;
  }
  .f-name {
    flex: 1;
    font-weight: 600;
    font-size: var(--text-sm);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text-primary);
    min-width: 0;
  }
  .file-confirm-list.single .f-name {
    flex: none;
    max-width: 100%;
    font-size: 13.5px;
  }
  .f-size {
    font-size: 11.5px;
    color: var(--text-muted);
    flex-shrink: 0;
  }
  .f-remove {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    color: var(--text-muted);
    flex-shrink: 0;
    transition:
      background var(--transition),
      color var(--transition);
  }
  @media (hover: hover) {
    .f-remove:hover:not(:disabled) {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
  }
  .f-remove:disabled {
    opacity: 0.4;
  }
  .f-total {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }
  .fc-caption {
    width: 100%;
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    color: var(--text-primary);
    font-family: inherit;
    font-size: 13.5px;
    line-height: 1.4;
    padding: 9px 12px;
    resize: none;
    max-height: 96px;
    overflow-y: hidden;
    scrollbar-width: thin;
    transition: border-color var(--transition);
  }
  .fc-caption::placeholder {
    color: var(--text-muted);
  }
  .fc-caption:focus {
    outline: none;
    border-color: var(--accent);
  }
  .fc-caption:disabled {
    opacity: 0.5;
  }
  .file-confirm-actions {
    display: flex;
    gap: 8px;
    width: 100%;
    margin-top: 2px;
  }
  .fc-btn {
    flex: 1;
    padding: 10px 14px;
    border-radius: var(--radius-md);
    font-weight: 600;
    font-size: 13.5px;
    transition: all var(--transition);
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 38px;
  }
  .fc-btn.ghost {
    background: var(--bg-tertiary);
    color: var(--text-secondary);
    border: 1px solid var(--border);
  }
  @media (hover: hover) {
    .fc-btn.ghost:hover:not(:disabled) {
      background: var(--bg-hover);
      color: var(--text-primary);
    }
  }
  .fc-btn.primary {
    background: var(--accent);
    color: #fff;
  }
  @media (hover: hover) {
    .fc-btn.primary:hover:not(:disabled) {
      background: var(--accent-hover);
    }
  }
  .fc-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
