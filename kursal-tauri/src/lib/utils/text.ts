export const ELLIPSIS = '…';

export function truncate(value: string, maxLen: number): string {
  if (value.length <= maxLen) return value;
  return value.slice(0, maxLen - 1) + ELLIPSIS;
}

// Keeps the extension visible, so "a-very-long-report.pdf" stays recognisable.
export function midTruncate(name: string, maxLen = 30): string {
  if (name.length <= maxLen) return name;
  const dot = name.lastIndexOf('.');
  const ext = dot > 0 && name.length - dot < 8 ? name.slice(dot) : '';
  const stem = ext ? name.slice(0, name.length - ext.length) : name;
  const room = maxLen - ext.length - 1;
  if (room < 6) return name.slice(0, maxLen - 1) + ELLIPSIS;
  const head = Math.ceil(room * 0.6);
  const tail = room - head;
  return stem.slice(0, head) + ELLIPSIS + stem.slice(stem.length - tail) + ext;
}

export function shortenId(id: string, head = 6, tail = 4): string {
  if (id.length <= head + tail) return id;
  return `${id.slice(0, head)}${ELLIPSIS}${id.slice(-tail)}`;
}

export function basename(path: string): string {
  const i = Math.max(path.lastIndexOf('/'), path.lastIndexOf('\\'));
  return i >= 0 ? path.slice(i + 1) : path;
}

export function extensionOf(filename: string): string {
  const dot = filename.lastIndexOf('.');
  return dot < 0 ? '' : filename.slice(dot + 1).toLowerCase();
}
