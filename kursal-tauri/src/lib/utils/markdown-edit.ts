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

const FENCE_RE = /^ {0,3}(`{3,})(.*)$/;

/** The fence the caret is writing inside, or null. Its opening line counts as inside. */
export function openFenceAt(value: string, caret: number): { lang: string } | null {
  let lang: string | null = null;
  for (const line of value.slice(0, caret).split('\n')) {
    const match = FENCE_RE.exec(line);
    if (match) lang = lang === null ? match[2].trim() : null;
  }
  return lang === null ? null : { lang };
}

/** Character spans of every fenced block, fence lines included. An unclosed one runs to the end. */
export function fenceRanges(value: string): Array<{ start: number; end: number }> {
  const ranges: Array<{ start: number; end: number }> = [];
  let offset = 0;
  let open: number | null = null;

  for (const line of value.split('\n')) {
    if (FENCE_RE.test(line)) {
      if (open === null) open = offset;
      else {
        ranges.push({ start: open, end: offset + line.length });
        open = null;
      }
    }
    offset += line.length + 1;
  }

  if (open !== null) ranges.push({ start: open, end: value.length });
  return ranges;
}

/** True when the caret rests at the end of a line that opens a block, info string and all. */
export function opensFence(value: string, caret: number): boolean {
  const line = value.slice(0, caret).split('\n').pop() ?? '';
  if (!/^ {0,3}`{3}[\w+#.-]*$/.test(line)) return false;
  const rest = value.slice(caret);
  if (rest !== '' && !rest.startsWith('\n')) return false;
  return openFenceAt(value, caret) !== null;
}

/** Closes a freshly opened fence, leaving the caret on the empty line inside it. */
export function computeFenceBody(caret: number): TextEdit {
  return {
    start: caret,
    end: caret,
    text: '\n\n```',
    selStart: caret + 1,
    selEnd: caret + 1,
  };
}

// A selection spanning lines wants a block; anything shorter wants inline code.
export function computeCode(value: string, start: number, end: number): TextEdit {
  const sel = value.slice(start, end);
  if (!sel.includes('\n')) return computeWrap(value, start, end, '`');

  const before = value.slice(0, start);
  const after = value.slice(end);
  const open = (before === '' || before.endsWith('\n') ? '' : '\n') + '```\n';
  const close = '\n```' + (after === '' || after.startsWith('\n') ? '' : '\n');
  return {
    start,
    end,
    text: open + sel + close,
    selStart: start + open.length,
    selEnd: start + open.length + sel.length,
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
