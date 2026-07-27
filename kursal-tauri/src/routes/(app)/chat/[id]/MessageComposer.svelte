<script lang="ts">
  import { tick } from 'svelte';
  import {
    Send,
    X,
    Paperclip,
    Smile,
    Bold,
    Italic,
    Strikethrough,
    Code,
    EyeOff,
    Link,
  } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import Spinner from '$lib/components/Spinner.svelte';
  import EmojiPicker from '$lib/components/EmojiPicker.svelte';
  import ShortcodeAutocomplete from './ShortcodeAutocomplete.svelte';
  import { loadEmojiIndex, searchEmojis, applyTone, getTone, type Emoji } from '$lib/emoji';
  import type { ContactResponse } from '$lib/types';

  interface Props {
    contact: ContactResponse;
    terminated: boolean;
    inputText: string;
    sending: boolean;
    isCoarsePointer: boolean;
    replyingPreview: string;
    editingPreview: string;
    replyActive: boolean;
    editActive: boolean;
    onSend: () => void;
    onAttach: () => void;
    onInput: () => void;
    onCancelReply: () => void;
    onCancelEdit: () => void;
    onEditLast: () => void;
    onOpenProfile: () => void;
    onPasteImage: (data: { bytes: Uint8Array; ext: string }) => void;
    composerEl?: HTMLTextAreaElement | null;
  }

  let {
    contact,
    terminated,
    inputText = $bindable(),
    sending,
    isCoarsePointer,
    replyingPreview,
    editingPreview,
    replyActive,
    editActive,
    onSend,
    onAttach,
    onInput,
    onCancelReply,
    onCancelEdit,
    onEditLast,
    onOpenProfile,
    onPasteImage,
    composerEl = $bindable(null),
  }: Props = $props();

  const MAX_MESSAGE_LENGTH = 10000;
  const nearLimit = $derived(inputText.length > MAX_MESSAGE_LENGTH - 500);
  let showEmoji = $state(false);
  let emojiBtnEl = $state<HTMLButtonElement | null>(null);
  let emojiBtnRect = $state<DOMRect | null>(null);
  let formatBarVisible = $state(false);
  let stickyFormatBar = $state(false);
  let popoverPos = $state<{ top: number; left: number } | null>(null);
  let composerRootEl = $state<HTMLDivElement | null>(null);

  // Picker lives in a fixed layer (escapes the composer's backdrop-filter
  // clipping) and anchors by its BOTTOM edge above the button, so shrinking
  // (fewer search results) collapses it upward, staying glued to the button.
  const EMOJI_PICKER_W = 352;
  const emojiPickerPos = $derived.by(
    (): {
      top?: number;
      bottom?: number;
      left: number;
    } | null => {
      if (!emojiBtnRect) return null;
      const r = emojiBtnRect;
      const margin = 8;
      const vw = window.innerWidth;
      // Use the visual viewport height so the picker stays above an open
      // on-screen keyboard instead of being anchored behind it.
      const vh = window.visualViewport?.height ?? window.innerHeight;
      let left = r.left;
      left = Math.max(margin, Math.min(left, vw - EMOJI_PICKER_W - margin));
      const spaceAbove = r.top - margin * 2;
      if (spaceAbove >= 220) {
        return { bottom: vh - (r.top - margin), left };
      }
      return { top: r.bottom + margin, left };
    }
  );

  function toggleEmoji(e: MouseEvent) {
    if (showEmoji) {
      showEmoji = false;
      return;
    }
    emojiBtnRect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    showEmoji = true;
    // On touch devices the keyboard would otherwise cover the picker; dismiss
    // it so the picker takes the space where the keyboard was.
    if (isCoarsePointer) composerEl?.blur();
  }

  // While the picker is open, re-anchor it to the button whenever the visual
  // viewport changes size (the keyboard finishing its open/close animation
  // moves the composer), so it never ends up floating away from the button.
  $effect(() => {
    if (!showEmoji) return;
    const vv = window.visualViewport;
    const reanchor = () => {
      if (emojiBtnEl) emojiBtnRect = emojiBtnEl.getBoundingClientRect();
    };
    vv?.addEventListener('resize', reanchor);
    vv?.addEventListener('scroll', reanchor);
    return () => {
      vv?.removeEventListener('resize', reanchor);
      vv?.removeEventListener('scroll', reanchor);
    };
  });

  let scFlat = $state<Emoji[] | null>(null);
  let scByShortcode = $state<Map<string, Emoji> | null>(null);
  loadEmojiIndex().then((i) => {
    scFlat = i.flat;
    scByShortcode = i.byShortcode;
  });
  let shortcodeQuery = $state<string | null>(null);
  let scSelected = $state(0);
  let scPos = $state<{ bottom: number; left: number } | null>(null);
  const scItems = $derived(
    shortcodeQuery && scFlat ? searchEmojis(scFlat, shortcodeQuery).slice(0, 8) : []
  );

  function acceptShortcode() {
    const item = scItems[scSelected];
    if (!item || !composerEl) return;
    const caret = composerEl.selectionStart;
    const start = caret - (shortcodeQuery!.length + 1);
    const u = applyTone(item, getTone());
    replaceRange(start, caret, u, start + u.length, start + u.length);
    shortcodeQuery = null;
  }
  // Suppress selection-driven position updates while we are mid-edit so
  // the popover doesn't jump between execCommand's collapsed caret and
  // the re-selected inner range.
  let isApplyingFormat = false;

  // Replace the textarea's [start..end] range with `text`.
  // Uses execCommand('insertText') so the change lands in the native
  // undo stack (Ctrl/Cmd+Z works), then sets the new selection.
  function replaceRange(
    start: number,
    end: number,
    text: string,
    selStart: number,
    selEnd: number
  ) {
    if (!composerEl) return;
    const el = composerEl;
    isApplyingFormat = true;
    el.focus();
    el.setSelectionRange(start, end);
    const ok = document.execCommand('insertText', false, text);
    if (!ok) {
      const v = el.value;
      el.value = v.slice(0, start) + text + v.slice(end);
      el.dispatchEvent(new Event('input', { bubbles: true }));
    }
    requestAnimationFrame(() => {
      el.setSelectionRange(selStart, selEnd);
      isApplyingFormat = false;
      // Single position update once selection is final.
      if (formatBarVisible) positionPopoverAtSelection();
    });
  }

  function applyWrap(prefix: string, suffix: string = prefix) {
    if (!composerEl) return;
    const el = composerEl;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    const value = el.value;
    const before = value.slice(0, start);
    const sel = value.slice(start, end);
    const after = value.slice(end);
    const wrapped = before.endsWith(prefix) && after.startsWith(suffix);
    if (wrapped) {
      // Strip surrounding markers
      replaceRange(
        start - prefix.length,
        end + suffix.length,
        sel,
        start - prefix.length,
        end - prefix.length
      );
    } else {
      replaceRange(start, end, prefix + sel + suffix, start + prefix.length, end + prefix.length);
    }
  }

  function applyLink() {
    if (!composerEl) return;
    const el = composerEl;
    const start = el.selectionStart;
    const end = el.selectionEnd;
    const sel = el.value.slice(start, end) || 'text';
    const inserted = `[${sel}](url)`;
    const urlStart = start + sel.length + 3;
    replaceRange(start, end, inserted, urlStart, urlStart + 3);
  }

  // Mirror-div trick: copy textarea styles into a hidden div,
  // splice a marker span at the caret, read its rect, position popover.
  function getCaretCoords(
    el: HTMLTextAreaElement,
    pos: number
  ): { left: number; top: number; height: number } {
    const styles = window.getComputedStyle(el);
    const div = document.createElement('div');
    const props = [
      'boxSizing',
      'width',
      'height',
      'borderTopWidth',
      'borderRightWidth',
      'borderBottomWidth',
      'borderLeftWidth',
      'borderStyle',
      'paddingTop',
      'paddingRight',
      'paddingBottom',
      'paddingLeft',
      'fontStyle',
      'fontVariant',
      'fontWeight',
      'fontStretch',
      'fontSize',
      'fontSizeAdjust',
      'lineHeight',
      'fontFamily',
      'textAlign',
      'textTransform',
      'textIndent',
      'textDecoration',
      'letterSpacing',
      'wordSpacing',
      'tabSize',
      'MozTabSize',
    ];
    const styleTarget = div.style as unknown as Record<string, string>;
    const styleSource = styles as unknown as Record<string, string>;
    for (const p of props) styleTarget[p] = styleSource[p];
    div.style.position = 'absolute';
    div.style.visibility = 'hidden';
    div.style.whiteSpace = 'pre-wrap';
    div.style.wordWrap = 'break-word';
    div.style.top = '0';
    div.style.left = '-9999px';
    div.style.overflow = 'hidden';
    div.textContent = el.value.slice(0, pos);
    const span = document.createElement('span');
    span.textContent = el.value.slice(pos) || '.';
    div.appendChild(span);
    document.body.appendChild(div);
    const spanRect = span.getBoundingClientRect();
    const divRect = div.getBoundingClientRect();
    const lineHeight = parseFloat(styles.lineHeight) || parseFloat(styles.fontSize) * 1.4;
    document.body.removeChild(div);
    const taRect = el.getBoundingClientRect();
    return {
      left: taRect.left + (spanRect.left - divRect.left) - el.scrollLeft,
      top: taRect.top + (spanRect.top - divRect.top) - el.scrollTop,
      height: lineHeight,
    };
  }

  const POPOVER_W = 224;
  const POPOVER_H = 36;

  // The bar is `position: fixed`, so clamping it to the viewport lets it slide
  // out of the chat column and over the sidebar, which paints on top of it.
  // Clamp to the composer's own box instead - it never leaves the chat column.
  function clampPopoverLeft(desired: number): number {
    const margin = 8;
    let min = margin;
    let max = window.innerWidth - POPOVER_W - margin;
    const host = composerRootEl?.getBoundingClientRect();
    if (host) {
      min = Math.max(min, host.left);
      max = Math.min(max, host.right - POPOVER_W);
    }
    if (max < min) max = min;
    return Math.max(min, Math.min(desired, max));
  }

  function positionPopoverAtSelection() {
    if (!composerEl) {
      popoverPos = null;
      return;
    }
    const start = composerEl.selectionStart;
    const coords = getCaretCoords(composerEl, start);
    const margin = 8;
    const left = clampPopoverLeft(coords.left - POPOVER_W / 2);
    let top = coords.top - POPOVER_H - 6;
    if (top < margin) top = coords.top + coords.height + 6;
    popoverPos = { top, left };
  }

  function positionPopoverAt(x: number, y: number) {
    const margin = 8;
    const left = clampPopoverLeft(x - POPOVER_W / 2);
    let top = y - POPOVER_H - 6;
    if (top < margin) top = y + 6;
    popoverPos = { top, left };
  }

  function handleFormatClick(prefix: string, suffix: string = prefix) {
    stickyFormatBar = true;
    applyWrap(prefix, suffix);
  }

  function handleLinkClick() {
    stickyFormatBar = true;
    applyLink();
  }

  function updateFormatBar() {
    if (isApplyingFormat) return;
    if (!composerEl) return;
    if (document.activeElement !== composerEl) {
      if (!stickyFormatBar) formatBarVisible = false;
      return;
    }
    const has = composerEl.selectionEnd > composerEl.selectionStart;
    formatBarVisible = has || stickyFormatBar;
    if (formatBarVisible) positionPopoverAtSelection();
  }

  async function handlePaste(e: ClipboardEvent) {
    const items = e.clipboardData?.items;
    if (!items) return;
    for (const it of items) {
      if (it.type.startsWith('image/')) {
        e.preventDefault();
        const blob = it.getAsFile();
        if (!blob) return;
        const bytes = new Uint8Array(await blob.arrayBuffer());
        const ext = it.type.split('/')[1] || 'png';
        onPasteImage({ bytes, ext });
        return;
      }
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (shortcodeQuery && scItems.length) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        scSelected = (scSelected + 1) % scItems.length;
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        scSelected = (scSelected - 1 + scItems.length) % scItems.length;
        return;
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault();
        acceptShortcode();
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        shortcodeQuery = null;
        return;
      }
    }

    if (e.key === 'ArrowUp' && inputText === '' && !shortcodeQuery && !editActive && !replyActive) {
      e.preventDefault();
      onEditLast();
      return;
    }

    if (e.key === 'Escape') {
      if (showEmoji) {
        showEmoji = false;
        return;
      }
      if (formatBarVisible) {
        formatBarVisible = false;
        stickyFormatBar = false;
        return;
      }
      if (replyActive) {
        e.preventDefault();
        onCancelReply();
        return;
      }
      if (editActive) {
        e.preventDefault();
        onCancelEdit();
        return;
      }
    }

    const meta = e.metaKey || e.ctrlKey;
    if (meta && !e.altKey) {
      const key = e.key.toLowerCase();
      if (e.shiftKey) {
        if (key === 'x') {
          e.preventDefault();
          applyWrap('~~');
          return;
        }
        if (key === 'p') {
          e.preventDefault();
          applyWrap('||');
          return;
        }
        if (key === 'k') {
          e.preventDefault();
          applyLink();
          return;
        }
      } else {
        if (key === 'b') {
          e.preventDefault();
          applyWrap('**');
          return;
        }
        if (key === 'i') {
          e.preventDefault();
          applyWrap('_');
          return;
        }
        if (key === 'e') {
          e.preventDefault();
          applyWrap('`');
          return;
        }
      }
    }

    if (e.key === 'Enter' && !e.shiftKey && !isCoarsePointer && !shortcodeQuery) {
      e.preventDefault();
      onSend();
    }
  }

  function handleEmojiSelect(emoji: string) {
    inputText += emoji;
    showEmoji = false;
    tick().then(() => composerEl?.focus());
  }

  function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    stickyFormatBar = true;
    formatBarVisible = true;
    composerEl?.focus();
    positionPopoverAt(e.clientX, e.clientY);
  }

  function handleMouseDown(e: MouseEvent) {
    // Left click in textarea = user is moving caret / starting fresh selection.
    // Drop the sticky flag so an empty selection actually hides the bar.
    if (e.button === 0) stickyFormatBar = false;
  }

  function handleSelect() {
    if (composerEl) {
      const has = composerEl.selectionEnd > composerEl.selectionStart;
      if (!has) stickyFormatBar = false;
    }
    updateFormatBar();
  }

  const SC_RE = /(^|\s):([a-z0-9_+\-]{2,})$/;
  // A completed :shortcode: token (closing colon typed) → replace instantly.
  const SC_CLOSE_RE = /(^|\s):([a-z0-9_+\-]+):$/;

  function handleInput() {
    onInput();
    if (isApplyingFormat) return;
    if (formatBarVisible) {
      requestAnimationFrame(positionPopoverAtSelection);
    }
    if (!composerEl) return;
    const before = composerEl.value.slice(0, composerEl.selectionStart);

    // Exact `:name:` → emoji (e.g. ":orange:" becomes 🍊) the moment the
    // closing colon is typed, if the name is an exact shortcode match.
    const exact = SC_CLOSE_RE.exec(before);
    if (exact && scByShortcode) {
      const hit = scByShortcode.get(exact[2]);
      if (hit) {
        const caret = composerEl.selectionStart;
        const start = caret - (exact[2].length + 2); // include both colons
        const u = applyTone(hit, getTone());
        replaceRange(start, caret, u, start + u.length, start + u.length);
        shortcodeQuery = null;
        return;
      }
    }

    const m = SC_RE.exec(before);
    if (m) {
      const q = m[2];
      shortcodeQuery = q;
      scSelected = 0;
      const coords = getCaretCoords(composerEl, composerEl.selectionStart);
      const popW = 220;
      const margin = 8;
      let left = coords.left;
      left = Math.max(margin, Math.min(left, window.innerWidth - popW - margin));
      // Anchor the popup ABOVE the caret line so it grows upward and never
      // clips off the bottom of the viewport (the composer sits at screen bottom).
      const bottom = window.innerHeight - coords.top + 8;
      scPos = { bottom, left };
    } else {
      shortcodeQuery = null;
    }
  }

  function handleBlur() {
    setTimeout(() => {
      if (!composerEl) return;
      if (document.activeElement !== composerEl) {
        formatBarVisible = false;
        stickyFormatBar = false;
      }
    }, 120);
  }

  $effect(() => {
    function onSel() {
      if (!composerEl) return;
      if (document.activeElement !== composerEl) return;
      updateFormatBar();
    }
    function onResize() {
      if (formatBarVisible) positionPopoverAtSelection();
    }
    document.addEventListener('selectionchange', onSel);
    window.addEventListener('resize', onResize);
    return () => {
      document.removeEventListener('selectionchange', onSel);
      window.removeEventListener('resize', onResize);
    };
  });
</script>

<div class="composer" bind:this={composerRootEl}>
  {#if contact.blocked}
    <div class="blocked-bar">
      <span>{t('chat.composer.blocked')}</span>
      <button class="blocked-link" onclick={onOpenProfile}>{t('chat.composer.manage')}</button>
    </div>
  {:else if terminated}
    <div class="blocked-bar">
      <span>{t('chat.composer.terminated', { name: contact.displayName })}</span>
      <button class="blocked-link" onclick={onOpenProfile}>{t('chat.composer.manage')}</button>
    </div>
  {:else}
    {#if replyActive}
      <div class="composer-context">
        <span class="ctx-bar"></span>
        <div class="ctx-body">
          <span class="ctx-label">{t('chat.composer.contextReplying')}</span>
          <span class="ctx-preview">{replyingPreview}</span>
        </div>
        <button
          class="ctx-cancel"
          onclick={onCancelReply}
          aria-label={t('chat.composer.cancelReplyAriaLabel')}
          disabled={sending}
        >
          <X size={14} />
        </button>
      </div>
    {:else if editActive}
      <div class="composer-context editing">
        <span class="ctx-bar"></span>
        <div class="ctx-body">
          <span class="ctx-label">{t('chat.composer.contextEditing')}</span>
          <span class="ctx-preview">{editingPreview}</span>
        </div>
        <button
          class="ctx-cancel"
          onclick={onCancelEdit}
          aria-label={t('chat.composer.cancelEditAriaLabel')}
          disabled={sending}
        >
          <X size={14} />
        </button>
      </div>
    {/if}

    <div class="composer-row">
      <button
        class="composer-btn"
        title={t('chat.composer.attachFile')}
        aria-label={t('chat.composer.attachFile')}
        onclick={onAttach}
        disabled={sending}
      >
        <Paperclip size={18} />
      </button>

      <div class="emoji-compose-anchor">
        <button
          bind:this={emojiBtnEl}
          class="composer-btn"
          title={t('chat.composer.emoji')}
          aria-label={t('chat.composer.emoji')}
          onmousedown={(e) => e.stopPropagation()}
          onclick={toggleEmoji}
          disabled={sending}
        >
          <Smile size={18} />
        </button>
      </div>

      <textarea
        bind:this={composerEl}
        bind:value={inputText}
        oninput={handleInput}
        onkeydown={handleKeydown}
        oncontextmenu={handleContextMenu}
        onmousedown={handleMouseDown}
        onselect={handleSelect}
        onblur={handleBlur}
        onpaste={handlePaste}
        placeholder={editActive
          ? t('chat.composer.placeholderEditing')
          : replyActive
            ? t('chat.composer.placeholderReplying')
            : t('chat.composer.placeholder', { name: contact.displayName })}
        rows="1"
        maxlength={MAX_MESSAGE_LENGTH}
        disabled={sending}></textarea>

      {#if nearLimit}
        <span class="char-count" class:over={inputText.length >= MAX_MESSAGE_LENGTH}
          >{MAX_MESSAGE_LENGTH - inputText.length}</span
        >
      {/if}

      <button
        class="send-btn"
        class:ready={!!inputText.trim() && !sending}
        onclick={onSend}
        disabled={!inputText.trim() || sending}
        aria-label={t('chat.composer.sendAriaLabel')}
      >
        {#if sending}
          <Spinner size={16} color="#fff" />
        {:else}
          <Send size={16} />
        {/if}
      </button>
    </div>
  {/if}
</div>

{#if showEmoji}
  <div
    class="emoji-compose-layer"
    style={emojiPickerPos
      ? emojiPickerPos.bottom !== undefined
        ? `bottom:${emojiPickerPos.bottom}px;left:${emojiPickerPos.left}px;`
        : `top:${emojiPickerPos.top}px;left:${emojiPickerPos.left}px;`
      : `bottom:84px;left:8px;`}
  >
    <EmojiPicker
      onSelect={handleEmojiSelect}
      onClose={() => (showEmoji = false)}
      autoFocus={!isCoarsePointer}
    />
  </div>
{/if}

{#if shortcodeQuery && scItems.length && scPos}
  <ShortcodeAutocomplete
    items={scItems}
    bottom={scPos.bottom}
    left={scPos.left}
    bind:selected={scSelected}
    onPick={(u) => {
      if (!composerEl) return;
      const caret = composerEl.selectionStart;
      const start = caret - (shortcodeQuery!.length + 1);
      replaceRange(start, caret, u, start + u.length, start + u.length);
      shortcodeQuery = null;
    }}
  />
{/if}

{#if formatBarVisible && popoverPos}
  <div
    class="format-bar-floating"
    role="toolbar"
    tabindex="0"
    aria-label={t('chat.composer.markdownAria')}
    style="top: {popoverPos.top}px; left: {popoverPos.left}px;"
    onmousedown={(e) => e.preventDefault()}
  >
    <button
      class="fmt-btn"
      title="{t('chat.composer.formatBold')}  (⌘B)"
      aria-label={t('chat.composer.formatBold')}
      onmousedown={(e) => e.preventDefault()}
      onclick={() => handleFormatClick('**')}
    >
      <Bold size={14} />
    </button>
    <button
      class="fmt-btn"
      title="{t('chat.composer.formatItalic')}  (⌘I)"
      aria-label={t('chat.composer.formatItalic')}
      onmousedown={(e) => e.preventDefault()}
      onclick={() => handleFormatClick('_')}
    >
      <Italic size={14} />
    </button>
    <button
      class="fmt-btn"
      title="{t('chat.composer.formatStrike')}  (⌘⇧X)"
      aria-label={t('chat.composer.formatStrike')}
      onmousedown={(e) => e.preventDefault()}
      onclick={() => handleFormatClick('~~')}
    >
      <Strikethrough size={14} />
    </button>
    <button
      class="fmt-btn"
      title="{t('chat.composer.formatCode')}  (⌘E)"
      aria-label={t('chat.composer.formatCode')}
      onmousedown={(e) => e.preventDefault()}
      onclick={() => handleFormatClick('`')}
    >
      <Code size={14} />
    </button>
    <button
      class="fmt-btn"
      title="{t('chat.composer.formatSpoiler')}  (⌘⇧P)"
      aria-label={t('chat.composer.formatSpoiler')}
      onmousedown={(e) => e.preventDefault()}
      onclick={() => handleFormatClick('||')}
    >
      <EyeOff size={14} />
    </button>
    <button
      class="fmt-btn"
      title="{t('chat.composer.formatLink')}  (⌘K)"
      aria-label={t('chat.composer.formatLink')}
      onmousedown={(e) => e.preventDefault()}
      onclick={handleLinkClick}
    >
      <Link size={14} />
    </button>
  </div>
{/if}

<style>
  .composer {
    background: var(--bg-secondary);
    backdrop-filter: blur(20px) saturate(140%);
    -webkit-backdrop-filter: blur(20px) saturate(140%);
    padding: 4px 8px;
    position: relative;
    flex-shrink: 0;
    border-radius: var(--radius-lg);
    border: 1px solid var(--border-light);
    box-shadow:
      0 1px 2px rgba(0, 0, 0, 0.04),
      0 8px 24px rgba(0, 0, 0, 0.08);
    pointer-events: auto;
  }
  /* Translucency only where color-mix() exists: a color-mix() value containing
     var() computes to `transparent` on engines without it (Chrome < 111). */
  @supports (background: color-mix(in srgb, red 50%, transparent)) {
    .composer {
      background: color-mix(in srgb, var(--bg-secondary) 80%, transparent);
    }
  }
  .composer:focus-within {
    border-color: var(--accent-selected);
  }

  .format-bar-floating {
    position: fixed;
    z-index: 1000;
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 3px;
    background: var(--surface);
    backdrop-filter: blur(14px) saturate(140%);
    -webkit-backdrop-filter: blur(14px) saturate(140%);
    border: 1px solid var(--border-light);
    border-radius: var(--radius-md);
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.18);
    width: fit-content;
    animation: fmt-in 160ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  @keyframes fmt-in {
    from {
      opacity: 0;
      transform: translateY(4px) scale(0.94);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }
  .fmt-btn {
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    transition: all var(--transition);
  }
  .fmt-btn:hover {
    background: var(--bg-hover);
    color: var(--text-primary);
  }
  .fmt-btn:active {
    transform: scale(0.92);
  }

  .composer-context {
    display: flex;
    align-items: stretch;
    gap: 8px;
    padding: 6px 10px;
    margin-bottom: 6px;
    background: var(--accent-dim);
    border-radius: var(--radius-md);
    font-size: 12.5px;
    animation: slideUp 0.15s ease-out;
  }
  .composer-context.editing {
    background: rgba(251, 191, 36, 0.12);
  }
  .ctx-bar {
    width: 3px;
    background: var(--accent);
    border-radius: 2px;
  }
  .composer-context.editing .ctx-bar {
    background: var(--warning);
  }
  .ctx-body {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    justify-content: center;
  }
  .ctx-label {
    font-weight: 700;
    color: var(--accent-hover);
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.02em;
  }
  .composer-context.editing .ctx-label {
    color: var(--warning);
  }
  .ctx-preview {
    color: var(--text-muted);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }
  .ctx-cancel {
    width: 26px;
    height: 26px;
    align-self: center;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    transition: all var(--transition);
    flex-shrink: 0;
  }
  .ctx-cancel:hover {
    background: rgba(248, 113, 113, 0.15);
    color: var(--danger);
  }

  @keyframes slideUp {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .composer-row {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    padding: 0;
    transition: border-color var(--transition);
  }

  .composer-btn {
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    border-radius: 50%;
    transition: all var(--transition);
    flex-shrink: 0;
  }
  .composer-btn:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--bg-hover);
  }
  .composer-btn:active:not(:disabled) {
    transform: scale(0.92);
  }
  .composer-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .emoji-compose-anchor {
    position: relative;
  }
  .emoji-compose-layer {
    position: fixed;
    z-index: 250;
    animation: pickerFadeIn 0.12s ease;
  }
  @keyframes pickerFadeIn {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  textarea {
    flex: 1;
    background: transparent;
    border: none;
    color: var(--text-primary);
    padding: 6px 6px;
    resize: none;
    min-height: 32px;
    max-height: 160px;
    font-size: 14.5px;
    line-height: 1.35;
    outline: none;
    font-family: inherit;
  }
  textarea::placeholder {
    color: var(--text-muted);
  }
  textarea:disabled {
    opacity: 0.5;
  }

  .char-count {
    align-self: center;
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
    padding: 0 4px;
    flex-shrink: 0;
  }
  .char-count.over {
    color: var(--danger);
    font-weight: 700;
  }

  .send-btn {
    width: 32px;
    height: 32px;
    background: transparent;
    color: var(--text-muted);
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    transition:
      background 200ms cubic-bezier(0.34, 1.56, 0.64, 1),
      color 200ms ease,
      transform 200ms cubic-bezier(0.34, 1.56, 0.64, 1);
    flex-shrink: 0;
    transform: scale(0.9);
    opacity: 0.7;
  }
  .send-btn.ready {
    background: var(--accent);
    color: #fff;
    transform: scale(1);
    opacity: 1;
  }
  .send-btn.ready:hover {
    background: var(--accent-hover);
    transform: scale(1.06);
  }
  .send-btn:active:not(:disabled) {
    transform: scale(0.92);
  }
  .send-btn:disabled {
    cursor: not-allowed;
  }
  .send-btn :global(svg) {
    transition: transform 200ms cubic-bezier(0.34, 1.56, 0.64, 1);
  }
  .send-btn.ready :global(svg) {
    transform: translateX(1px);
  }

  .blocked-bar {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 14px;
    color: var(--text-muted);
    font-size: 13px;
  }
  .blocked-link {
    color: var(--accent);
    font-size: 13px;
    font-weight: 600;
    padding: 0;
  }
  .blocked-link:hover {
    text-decoration: underline;
  }

  @media (max-width: 768px) {
    .composer {
      padding-left: 8px;
      padding-right: 8px;
    }
    textarea {
      font-size: 16px;
    }
  }
</style>
