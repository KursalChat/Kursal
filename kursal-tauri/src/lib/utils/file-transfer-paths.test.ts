import { describe, it, expect, vi, beforeEach } from 'vitest';

// file-transfer-paths imports Tauri plugins at module load; stub them so the
// helpers under test can be imported in isolation.
const writeFile = vi.fn(async () => {});
const readFile = vi.fn(async () => new Uint8Array());
const copyFile = vi.fn(async () => {});

let slot = 0;
const createOutgoingPendingPath = vi.fn(async (filename: string) => {
  slot += 1;
  return `/data/outgoing/pending/${slot}/${filename}`;
});

vi.mock('@tauri-apps/plugin-fs', () => ({ copyFile, readFile, writeFile }));
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock('$lib/api/messages', () => ({
  createOutgoingPendingPath,
  resolveDownloadPath: vi.fn(),
  availableSpace: vi.fn(),
}));
vi.mock('$lib/api/window', () => ({ isMobile: false, OS: 'macos' }));

const {
  filenameFromPath,
  prepareOfferFromBytes,
  prepareOfferSourcePath,
  needsDestinationPrompt,
  LARGE_FILE_PROMPT_BYTES,
} = await import('./file-transfer-paths');

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

describe('prepareOfferSourcePath', () => {
  beforeEach(() => {
    readFile.mockClear();
    writeFile.mockClear();
  });

  // A drop must never be read or copied by the webview: a 1 GB file has to
  // cost nothing until the core streams it.
  it('offers a dropped file from its original path without touching the bytes', async () => {
    const prepared = await prepareOfferSourcePath('/home/user/holiday.png');

    expect(prepared.backendPath).toBe('/home/user/holiday.png');
    expect(prepared.filename).toBe('holiday.png');
    expect(readFile).not.toHaveBeenCalled();
    expect(writeFile).not.toHaveBeenCalled();
  });

  it('normalizes a file:// URI to a local path', async () => {
    const prepared = await prepareOfferSourcePath('file:///home/user/my%20file.txt');
    expect(prepared.backendPath).toBe('/home/user/my file.txt');
  });
});

describe('pending staging', () => {
  beforeEach(() => {
    writeFile.mockClear();
    createOutgoingPendingPath.mockClear();
  });

  // The core offers the peer whatever the staged path's basename is, so the
  // name must survive staging untouched; uniqueness lives in the directory.
  it('keeps the filename as the basename and makes the directory unique', async () => {
    const a = await prepareOfferFromBytes(new Uint8Array([1, 2, 3]), 'holiday.png');
    const b = await prepareOfferFromBytes(new Uint8Array([4, 5, 6]), 'holiday.png');

    expect(a.filename).toBe('holiday.png');
    expect(a.backendPath.split('/').pop()).toBe('holiday.png');
    expect(b.backendPath.split('/').pop()).toBe('holiday.png');
    expect(a.backendPath).not.toBe(b.backendPath);
  });

  it('writes the bytes to the slot the core handed back', async () => {
    const staged = await prepareOfferFromBytes(new Uint8Array([1]), 'note.txt');

    expect(createOutgoingPendingPath).toHaveBeenCalledWith('note.txt');
    expect(writeFile).toHaveBeenCalledWith(staged.backendPath, new Uint8Array([1]));
  });
});

describe('needsDestinationPrompt', () => {
  const GB = 1024 * 1024 * 1024;

  it('sends an ordinary file to the download folder', () => {
    expect(needsDestinationPrompt(4 * GB, 500 * GB)).toBe(false);
  });

  it('asks where to put a file past the threshold even with room for it', () => {
    expect(needsDestinationPrompt(LARGE_FILE_PROMPT_BYTES + 1, 500 * GB)).toBe(true);
  });

  it('asks when the download folder cannot hold the file', () => {
    expect(needsDestinationPrompt(2 * GB, GB)).toBe(true);
  });

  it('treats an exact fit as fitting', () => {
    expect(needsDestinationPrompt(GB, GB)).toBe(false);
  });

  // An unreadable free-space figure must not turn every accept into a picker.
  it('does not prompt on a size it cannot check', () => {
    expect(needsDestinationPrompt(2 * GB, null)).toBe(false);
  });
});
