import { describe, it, expect } from 'vitest';
import { formatBytes, formatFileSize, formatBytePair, bytesToMB, mbToBytes } from './bytes';

describe('formatBytes', () => {
  it('picks the largest unit that keeps a whole part', () => {
    expect(formatBytes(512)).toBe('512 B');
    expect(formatBytes(2048)).toBe('2.0 KB');
    expect(formatBytes(5 * 1024 * 1024)).toBe('5.0 MB');
    expect(formatBytes(3 * 1024 * 1024 * 1024)).toBe('3.00 GB');
  });

  it('stops at the requested maximum unit', () => {
    expect(formatBytes(3 * 1024 * 1024 * 1024, { maxUnit: 'MB' })).toBe('3072.0 MB');
    expect(formatBytes(5 * 1024 * 1024, { maxUnit: 'KB' })).toBe('5120.0 KB');
  });

  it('renders zero as "0 B" unless the caller wants it blank', () => {
    expect(formatBytes(0)).toBe('0 B');
    expect(formatFileSize(0)).toBe('');
    expect(formatFileSize(-1)).toBe('');
  });
});

describe('formatBytePair', () => {
  it('shares the larger value unit across both halves', () => {
    expect(formatBytePair(512 * 1024, 2 * 1024 * 1024)).toBe('0.5 / 2.0 MB');
  });

  it('drops the total when it is unknown', () => {
    expect(formatBytePair(2048, null)).toBe('2.0 KB');
  });
});

describe('MB conversion', () => {
  it('round-trips whole megabytes', () => {
    expect(bytesToMB(mbToBytes('12'))).toBe('12');
  });

  it('clamps junk input to zero', () => {
    expect(mbToBytes('-5')).toBe(0);
    expect(mbToBytes('abc')).toBe(0);
  });
});
