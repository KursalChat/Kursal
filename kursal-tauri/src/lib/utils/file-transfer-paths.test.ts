import { describe, it, expect, vi } from 'vitest';

// file-transfer-paths imports Tauri plugins at module load; stub them so the
// pure helper under test (filenameFromPath) can be imported in isolation.
vi.mock('@tauri-apps/api/path', () => ({ appCacheDir: vi.fn(), join: vi.fn() }));
vi.mock('@tauri-apps/plugin-fs', () => ({
  copyFile: vi.fn(),
  readFile: vi.fn(),
  remove: vi.fn(),
  writeFile: vi.fn(),
}));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock('$lib/api/window', () => ({ isMobile: false, OS: 'macos' }));

const { filenameFromPath } = await import('./file-transfer-paths');

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

  it('handles content:// URIs', () => {
    expect(filenameFromPath('content://media/external/file/1234')).toBe('1234');
  });
});
