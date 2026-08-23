export interface TextEdit {
  start: number;
  end: number;
  text: string;
  selStart: number;
  selEnd: number;
}

// Toggles: a selection already surrounded by the markers is unwrapped instead.
export function computeWrap(
  value: string,
  start: number,
  end: number,
  prefix: string,
  suffix: string = prefix
): TextEdit {
  const before = value.slice(0, start);
  const sel = value.slice(start, end);
  const after = value.slice(end);

  if (before.endsWith(prefix) && after.startsWith(suffix)) {
    return {
      start: start - prefix.length,
      end: end + suffix.length,
      text: sel,
      selStart: start - prefix.length,
      selEnd: end - prefix.length,
    };
  }
  return {
    start,
    end,
    text: prefix + sel + suffix,
    selStart: start + prefix.length,
    selEnd: end + prefix.length,
  };
}

export function computeLink(value: string, start: number, end: number): TextEdit {
  const sel = value.slice(start, end) || 'text';
  const urlStart = start + sel.length + 3;
  return {
    start,
    end,
    text: `[${sel}](url)`,
    selStart: urlStart,
    selEnd: urlStart + 3,
  };
}

// execCommand keeps the native undo stack intact; the manual path is the
// fallback for engines that have dropped it.
export function applyTextEdit(
  el: HTMLTextAreaElement,
  edit: TextEdit,
  onSettled?: () => void
): void {
  el.focus();
  el.setSelectionRange(edit.start, edit.end);
  const ok = document.execCommand('insertText', false, edit.text);
  if (!ok) {
    const v = el.value;
    el.value = v.slice(0, edit.start) + edit.text + v.slice(edit.end);
    el.dispatchEvent(new Event('input', { bubbles: true }));
  }
  requestAnimationFrame(() => {
    el.setSelectionRange(edit.selStart, edit.selEnd);
    onSettled?.();
  });
}
