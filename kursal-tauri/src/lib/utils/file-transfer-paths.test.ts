import { describe, it, expect, vi, beforeEach } from 'vitest';

// file-transfer-paths imports Tauri plugins at module load; stub them so the
// helpers under test can be imported in isolation.
const appCacheDir = vi.fn(async () => '/cache');
const join = vi.fn(async (...parts: string[]) => parts.join('/'));
const mkdir = vi.fn(async () => {});
const writeFile = vi.fn(async () => {});
const readFile = vi.fn(async () => new Uint8Array());
const copyFile = vi.fn(async () => {});

vi.mock('@tauri-apps/api/path', () => ({ appCacheDir, join }));
vi.mock('@tauri-apps/plugin-fs', () => ({ copyFile, mkdir, readFile, writeFile }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }));

const { filenameFromPath, prepareOfferFromBytes } = await import('./file-transfer-paths');

describe('filenameFromPath', () => {
  it("returns 'file' for empty input", () => {
    expect(filenameFromPath('')).toBe('file');
  });

  it('extracts the basename of a unix path', () => {
    expect(filenameFromPath('/home/user/photo.png')).toBe('photo.png');
  });

  it('extracts the basename of a windows path', () => {
    expect(filenameFromPath('C:\\Users\\me\\doc.pdf')).toBe('doc.pdf');
  });

  it('decodes and sanitizes a file:// URI', () => {
    expect(filenameFromPath('file:///home/user/my%20file.txt')).toBe('my_file.txt');
  });

  it('replaces unsafe characters with underscores', () => {
    expect(filenameFromPath('/tmp/a b*c?.txt')).toBe('a_b_c_.txt');
  });
});

describe('cache staging', () => {
  beforeEach(() => {
    mkdir.mockClear();
    writeFile.mockClear();
  });

  // The core offers the peer whatever the staged path's basename is, so the
  // name must survive staging untouched - uniqueness lives in the directory.
  it('keeps the filename as the basename and makes the directory unique', async () => {
    const a = await prepareOfferFromBytes(new Uint8Array([1, 2, 3]), 'holiday.png');
    const b = await prepareOfferFromBytes(new Uint8Array([4, 5, 6]), 'holiday.png');

    expect(a.filename).toBe('holiday.png');
    expect(a.backendPath.split('/').pop()).toBe('holiday.png');
    expect(b.backendPath.split('/').pop()).toBe('holiday.png');
    expect(a.backendPath).not.toBe(b.backendPath);
    expect(mkdir).toHaveBeenCalledTimes(2);
  });

  it('stages under the app cache', async () => {
    const staged = await prepareOfferFromBytes(new Uint8Array([1]), 'note.txt');
    expect(staged.backendPath.startsWith('/cache/outgoing/')).toBe(true);
  });
});
