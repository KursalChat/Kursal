import { describe, it, expect } from 'vitest';
import { truncate, midTruncate, shortenId, basename, extensionOf } from './text';

describe('truncate', () => {
  it('leaves short strings alone', () => {
    expect(truncate('hello', 10)).toBe('hello');
  });

  it('never exceeds the cap, ellipsis included', () => {
    const out = truncate('x'.repeat(50), 10);
    expect(out.length).toBe(10);
    expect(out.endsWith('…')).toBe(true);
  });
});

describe('midTruncate', () => {
  it('keeps the extension readable', () => {
    const out = midTruncate('a-really-long-file-name-here.pdf', 20);
    expect(out.length).toBeLessThanOrEqual(20);
    expect(out.endsWith('.pdf')).toBe(true);
    expect(out).toContain('…');
  });

  it('falls back to a plain cut when there is no room for a stem', () => {
    const out = midTruncate('averylongname.superlongextension', 8);
    expect(out.length).toBeLessThanOrEqual(8);
  });
});

describe('shortenId', () => {
  it('keeps ids that already fit', () => {
    expect(shortenId('abc', 6, 4)).toBe('abc');
  });

  it('elides the middle', () => {
    expect(shortenId('abcdefghijklmno', 6, 4)).toBe('abcdef…lmno');
  });
});

describe('path helpers', () => {
  it('takes the last segment of either separator', () => {
    expect(basename('/a/b/c.txt')).toBe('c.txt');
    expect(basename('C:\\a\\b\\c.txt')).toBe('c.txt');
    expect(basename('bare.txt')).toBe('bare.txt');
  });

  it('lowercases the extension and returns empty when absent', () => {
    expect(extensionOf('Photo.PNG')).toBe('png');
    expect(extensionOf('noext')).toBe('');
  });
});
