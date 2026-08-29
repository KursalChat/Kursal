export interface CaretCoords {
  left: number;
  top: number;
  height: number;
}

const MIRRORED_PROPS = [
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

// Mirror-div trick: mirrors the textarea's styles in a hidden div to read
// the caret's screen position.
export function getCaretCoords(el: HTMLTextAreaElement, pos: number): CaretCoords {
  const styles = window.getComputedStyle(el);
  const div = document.createElement('div');
  const styleTarget = div.style as unknown as Record<string, string>;
  const styleSource = styles as unknown as Record<string, string>;
  for (const p of MIRRORED_PROPS) styleTarget[p] = styleSource[p];
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
