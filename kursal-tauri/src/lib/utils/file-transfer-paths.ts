import { appCacheDir, join } from '@tauri-apps/api/path';
import { copyFile, mkdir, readFile, writeFile } from '@tauri-apps/plugin-fs';
import { open, save } from '@tauri-apps/plugin-dialog';
import { looksLikeStrippableImage, stripImageMetadata } from './image-metadata';

function isUriPath(path: string): boolean {
  return /^[a-z][a-z0-9+.-]*:\/\//i.test(path);
}

function pathFromFileUri(uri: string): string {
  const parsed = new URL(uri);
  let pathname = decodeURIComponent(parsed.pathname);
  if (/^\/[A-Za-z]:/.test(pathname)) pathname = pathname.slice(1);
  return pathname;
}

function sanitizeFilename(name: string): string {
  const trimmed = name.trim();
  const safe = trimmed.replace(/[^a-zA-Z0-9._-]/g, '_');
  return safe.length > 0 ? safe : 'file';
}

export function filenameFromPath(value: string): string {
  if (!value) return 'file';
  if (value.startsWith('file://')) {
    const localPath = pathFromFileUri(value);
    const part = localPath.split(/[\\/]/).pop();
    return part ? sanitizeFilename(part) : 'file';
  }
  if (isUriPath(value)) {
    try {
      const parsed = new URL(value);
      const part = decodeURIComponent(parsed.pathname).split('/').pop();
      return part ? sanitizeFilename(part) : 'file';
    } catch {
      return 'file';
    }
  }
  const part = value.split(/[\\/]/).pop();
  return part ? sanitizeFilename(part) : 'file';
}

/**
 * Stages outgoing bytes in the app cache under a fresh directory, keeping the
 * file's own name as the basename.
 *
 * The core derives the filename it offers to the peer from the basename of the
 * path handed to `send_file_offer`, so anything mixed into that basename here
 * (a timestamp, a random suffix) is what the recipient ends up seeing. The
 * uniqueness therefore lives in the directory name, never in the filename.
 */
async function stageBytesInCache(bytes: Uint8Array, filename: string): Promise<string> {
  const random = Math.floor(Math.random() * 1_000_000_000)
    .toString()
    .padStart(9, '0');
  const dir = await join(await appCacheDir(), 'outgoing', `${Date.now()}-${random}`);
  await mkdir(dir, { recursive: true });
  const path = await join(dir, sanitizeFilename(filename));
  await writeFile(path, bytes);
  return path;
}

function extractPath(raw: unknown): string {
  if (raw === null || raw === undefined) return '';
  if (typeof raw === 'string') return raw;
  if (Array.isArray(raw)) return extractPath(raw[0]);
  if (typeof raw === 'object') {
    const r = raw as { path?: unknown; toString?: () => string };
    if (typeof r.path === 'string') return r.path;
    if (typeof r.toString === 'function') return r.toString();
  }
  return String(raw);
}

export interface PreparedFile {
  backendPath: string;
  filename: string;
}

function extractPaths(raw: unknown): string[] {
  if (Array.isArray(raw)) {
    return raw.map((r) => extractPath(r)).filter((p) => p.length > 0);
  }
  const single = extractPath(raw);
  return single ? [single] : [];
}

/**
 * Desktop file picker. Mobile picks through the webview's own file inputs
 * (see AttachSheet), which hand back real File objects with real names rather
 * than opaque content:// ids.
 */
export async function pickFilesForSend(): Promise<PreparedFile[]> {
  const selected = await open({ multiple: true, directory: false });
  const raws = extractPaths(selected);
  return await Promise.all(raws.map((r) => prepareOfferSourcePath(r)));
}

/**
 * Images are re-written into the app cache without their metadata so EXIF/GPS
 * never leaves the device. Anything that isn't a strippable image, already
 * carries no metadata, or can't be read is offered from its original path.
 */
async function offerWithoutMetadata(localPath: string, filename: string): Promise<PreparedFile> {
  if (!looksLikeStrippableImage(filename)) return { backendPath: localPath, filename };
  try {
    const bytes = await readFile(localPath);
    const cleaned = stripImageMetadata(bytes);
    if (cleaned.length === bytes.length) return { backendPath: localPath, filename };
    return { backendPath: await stageBytesInCache(cleaned, filename), filename };
  } catch {
    return { backendPath: localPath, filename };
  }
}

/** Desktop picker selections and OS drag-and-drop payloads. */
export async function prepareOfferSourcePath(rawSelection: string): Promise<PreparedFile> {
  const filename = filenameFromPath(rawSelection);
  const localPath = rawSelection.startsWith('file://')
    ? pathFromFileUri(rawSelection)
    : rawSelection;
  return await offerWithoutMetadata(localPath, filename);
}

/** Webview File objects: mobile pickers, the camera input, pasted images. */
export async function prepareOfferFromFile(file: File, fallbackName: string): Promise<PreparedFile> {
  const filename = sanitizeFilename(file.name || fallbackName);
  const raw = new Uint8Array(await file.arrayBuffer());
  const bytes = looksLikeStrippableImage(filename) ? stripImageMetadata(raw) : raw;
  return { backendPath: await stageBytesInCache(bytes, filename), filename };
}

/** Pasted image bytes, which arrive without any name of their own. */
export async function prepareOfferFromBytes(
  bytes: Uint8Array,
  filename: string
): Promise<PreparedFile> {
  return {
    backendPath: await stageBytesInCache(stripImageMetadata(bytes), filename),
    filename,
  };
}

/**
 * Copies an already-downloaded file out of the app's storage to wherever the
 * user wants it. Downloads themselves never prompt - this is the explicit
 * "get it out of the app" action, which is the only way off mobile since app
 * storage isn't browsable there. Returns false when the picker is dismissed.
 */
export async function exportToDevice(sourcePath: string, filename: string): Promise<boolean> {
  const target = extractPath(await save({ defaultPath: sanitizeFilename(filename) }));
  if (!target) return false;
  try {
    await copyFile(sourcePath, target);
  } catch {
    // Android SAF targets (content://) aren't valid copy destinations.
    await writeFile(target, await readFile(sourcePath));
  }
  return true;
}
