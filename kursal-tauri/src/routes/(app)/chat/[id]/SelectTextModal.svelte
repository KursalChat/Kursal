<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { X, Code, Type } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import { renderMarkdown } from './chat-utils';

  interface Props {
    text: string;
    onClose: () => void;
  }

  let { text, onClose }: Props = $props();
  let textEl: HTMLDivElement | null = $state(null);
  let showRaw = $state(false);

  function selectAll() {
    void tick().then(() => {
      if (!textEl) return;
      const range = document.createRange();
      range.selectNodeContents(textEl);
      const sel = window.getSelection();
      sel?.removeAllRanges();
      sel?.addRange(range);
    });
  }

  function toggleRaw() {
    showRaw = !showRaw;
    selectAll();
  }

  onMount(() => {
    selectAll();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });
</script>

<div
  class="select-backdrop"
  onclick={onClose}
  onkeydown={(e) => {
    if (e.key === 'Escape') onClose();
  }}
  role="button"
  tabindex="-1"
  aria-label={t('chat.selectText.dialogAriaLabel')}
></div>
<div
  class="select-sheet"
  role="dialog"
  aria-modal="true"
  aria-label={t('chat.selectText.dialogAriaLabel')}
>
  <div class="select-head">
    <span class="select-title">{t('chat.selectText.title')}</span>
    <div class="select-actions">
      <button
        class="select-mode"
        onclick={toggleRaw}
        title={showRaw ? t('chat.selectText.viewFormatted') : t('chat.selectText.viewMarkdown')}
        aria-label={showRaw
          ? t('chat.selectText.viewFormatted')
          : t('chat.selectText.viewMarkdown')}
      >
        {#if showRaw}
          <Type size={15} /><span>{t('chat.selectText.viewFormatted')}</span>
        {:else}
          <Code size={15} /><span>{t('chat.selectText.viewMarkdown')}</span>
        {/if}
      </button>
      <button class="select-close" onclick={onClose} aria-label={t('chat.selectText.close')}>
        <X size={18} />
      </button>
    </div>
  </div>
  <div class="select-hint">{t('chat.selectText.hint')}</div>
  <div
    class="select-body"
    class:raw={showRaw}
    bind:this={textEl}
    role="textbox"
    aria-readonly="true"
    tabindex="-1"
  >
    {#if showRaw}
      {text}
    {:else}
      {@html renderMarkdown(text, false, false)}
    {/if}
  </div>
  <button class="select-all-btn" onclick={selectAll}>{t('chat.selectText.selectAll')}</button>
</div>

<style>
  .select-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    z-index: 320;
    animation: fadeIn 0.15s ease;
  }

  .select-sheet {
    position: fixed;
    top: var(--safe-center-y);
    left: var(--safe-center-x);
    transform: translate(-50%, -50%);
    width: min(520px, calc(var(--safe-w) - 32px));
    max-height: min(70vh, 560px, calc(var(--safe-h) - 32px));
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 14px;
    z-index: 330;
    animation: popIn 0.18s cubic-bezier(0.3, 0, 0.2, 1);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  @keyframes popIn {
    from {
      transform: translate(-50%, -50%) scale(0.94);
      opacity: 0;
    }
    to {
      transform: translate(-50%, -50%) scale(1);
      opacity: 1;
    }
  }

  .select-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 4px;
  }
  .select-title {
    font-size: var(--text-md);
    font-weight: 600;
    color: var(--text-primary);
  }
  .select-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .select-mode {
    display: flex;
    align-items: center;
    gap: 5px;
    height: 30px;
    padding: 0 10px;
    border-radius: var(--radius-md);
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-secondary);
    background: var(--bg-hover);
  }
  .select-mode:active {
    transform: scale(0.96);
  }
  .select-close {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
  }
  .select-close:active {
    background: var(--bg-hover);
  }

  .select-all-btn {
    margin-top: 10px;
    align-self: flex-end;
    height: 32px;
    padding: 0 14px;
    border-radius: var(--radius-md);
    font-size: var(--text-sm);
    font-weight: 600;
    color: #fff;
    background: var(--accent);
  }
  .select-all-btn:active {
    transform: scale(0.97);
  }

  .select-hint {
    font-size: var(--text-xs);
    color: var(--text-muted);
    margin-bottom: 8px;
  }

  .select-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    background: var(--bg-input);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 12px;
    color: var(--text-primary);
    font-size: var(--text-md);
    line-height: 1.5;
    word-break: break-word;
    user-select: text;
    -webkit-user-select: text;
    -webkit-touch-callout: default;
    outline: none;
  }

  .select-body.raw {
    white-space: pre-wrap;
    font-family: var(--font-mono);
    font-size: 13.5px;
  }
  .select-body :global(p) {
    margin: 0;
  }
  .select-body :global(p + p) {
    margin-top: 0.5em;
  }
  .select-body :global(code) {
    font-family: var(--font-mono);
    font-size: 0.88em;
  }
  /* Nothing listens for clicks in here, so the copy button would be inert. */
  .select-body :global(.md-code-bar) {
    display: none;
  }
  .select-body :global(pre code) {
    font-size: 1em;
  }
  .select-body :global(ul),
  .select-body :global(ol) {
    margin: 0.4em 0;
    padding-left: 1.3em;
  }
  .select-body :global(.spoiler) {
    filter: none;
  }
</style>
