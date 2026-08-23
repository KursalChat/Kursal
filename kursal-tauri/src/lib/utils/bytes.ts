const KB = 1024;
const MB = 1024 * 1024;
const GB = 1024 * 1024 * 1024;

export interface ByteUnit {
  unit: string;
  div: number;
  decimals: number;
}

export function byteUnitFor(bytes: number, maxUnit: 'KB' | 'MB' | 'GB' = 'GB'): ByteUnit {
  if (bytes >= GB && maxUnit === 'GB') return { unit: 'GB', div: GB, decimals: 2 };
  if (bytes >= MB && maxUnit !== 'KB') return { unit: 'MB', div: MB, decimals: 1 };
  if (bytes >= KB) return { unit: 'KB', div: KB, decimals: 1 };
  return { unit: 'B', div: 1, decimals: 0 };
}

export interface FormatBytesOptions {
  maxUnit?: 'KB' | 'MB' | 'GB';
  emptyForZero?: boolean;
}

export function formatBytes(bytes: number, options: FormatBytesOptions = {}): string {
  if (bytes <= 0) return options.emptyForZero ? '' : '0 B';
  const { unit, div, decimals } = byteUnitFor(bytes, options.maxUnit);
  return `${(bytes / div).toFixed(decimals)} ${unit}`;
}

// File sizes render as empty rather than "0 B": a zero-byte offer is still
// being negotiated and has no size to show yet.
export function formatFileSize(bytes: number): string {
  return formatBytes(bytes, { emptyForZero: true });
}

// Both sides share the larger value's unit, so the pair reads as one number.
export function formatBytePair(done: number, total: number | null | undefined): string {
  const { unit, div, decimals } = byteUnitFor(total ?? done, 'MB');
  const doneText = (done / div).toFixed(decimals);
  if (!total) return `${doneText} ${unit}`;
  return `${doneText} / ${(total / div).toFixed(decimals)} ${unit}`;
}

export function bytesToMB(bytes: number): string {
  return String(Math.round(bytes / MB));
}

export function mbToBytes(mb: string): number {
  return Math.max(0, Math.floor(Number(mb) || 0)) * MB;
}
