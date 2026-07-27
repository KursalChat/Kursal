import { appCacheDir, join } from '@tauri-apps/api/path';
import { copyFile, exists, readFile, remove, writeFile } from '@tauri-apps/plugin-fs';
import { open, save } from '@tauri-apps/plugin-dialog';
import { isMobile, OS } from '$lib/api/window';
import { looksLikeStrippableImage, stripImageMetadata } from './image-metadata';

const deferredDownloadTargets = new Map<string, { targetUri: string; filename: string }>();

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

async function writeBytesToAppCache(
  bytes: Uint8Array,
  suggestedFilename: string,
  prefix: string
): Promise<string> {
  const cacheDir = await appCacheDir();
  const timestamp = Date.now();
  const random = Math.floor(Math.random() * 1_000_000_000)
    .toString()
    .padStart(9, '0');
  const tempName = `${prefix}-${timestamp}-${random}-${sanitizeFilename(suggestedFilename)}`;
  const tempPath = await join(cacheDir, tempName);
  await writeFile(tempPath, bytes);
  return tempPath;
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

export type SendPickerMode = 'document' | 'media' | 'image' | 'video';

function extractPaths(raw: unknown): string[] {
  if (Array.isArray(raw)) {
    return raw.map((r) => extractPath(r)).filter((p) => p.length > 0);
  }
  const single = extractPath(raw);
  return single ? [single] : [];
}

export async function pickFilesForSend(
  pickerMode: SendPickerMode = 'document'
): Promise<PreparedFile[]> {
  try {
    const selected = await open({
      multiple: true,
      directory: false,
      pickerMode,
      fileAccessMode: 'copy',
    });
    const raws = extractPaths(selected);
    return await Promise.all(raws.map((r) => prepareOfferSourcePath(r)));
  } catch (e) {
    // Android: some builds reject unknown options - retry without them.
    if (isMobile) {
      const selected = await open({ multiple: true, directory: false });
      const raws = extractPaths(selected);
      return await Promise.all(raws.map((r) => prepareOfferSourcePath(r)));
    }
    throw e;
  }
}

export interface ResolvedReceivePath {
  backendPath: string;
  deferredTargetUri: string | null;
}

export async function pickFileForReceive(filename: string): Promise<ResolvedReceivePath | null> {
  const safeName = sanitizeFilename(filename);
  if (OS == 'android') {
    const backendPath = await writeBytesToAppCache(new Uint8Array(0), safeName, 'recv');
    return { backendPath, deferredTargetUri: null };
  }

  const savePath = await save({ defaultPath: safeName });
  const raw = extractPath(savePath);
  if (!raw) return null;
  return await resolveReceiveSavePath(raw, safeName);
}

async function uniqueReceivePath(
  dir: string,
  filename: string,
  claimed: Set<string>
): Promise<string> {
  const safe = sanitizeFilename(filename);
  const dot = safe.lastIndexOf('.');
  const base = dot > 0 ? safe.slice(0, dot) : safe;
  const ext = dot > 0 ? safe.slice(dot) : '';
  let candidate = await join(dir, safe);
  let n = 1;
  // Never clobber a file already on disk, nor one another item in this same
  // batch already claimed (two offers can carry the identical filename).
  while (claimed.has(candidate) || (await exists(candidate))) {
    candidate = await join(dir, `${base} (${n})${ext}`);
    n += 1;
  }
  claimed.add(candidate);
  return candidate;
}

/**
 * Resolves a save target for several incoming files at once. Desktop prompts
 * for one destination folder; mobile writes into the app cache like the
 * per-file receive path. Colliding names - with a file on disk or another item
 * in the same batch - get a " (n)" suffix so nothing is overwritten. Returns
 * null when the folder prompt is cancelled.
 */
export async function prepareBatchReceive(
  filenames: string[]
): Promise<ResolvedReceivePath[] | null> {
  if (OS == 'android') {
    const out: ResolvedReceivePath[] = [];
    for (const filename of filenames) {
      const backendPath = await writeBytesToAppCache(
        new Uint8Array(0),
        sanitizeFilename(filename),
        'recv'
      );
      out.push({ backendPath, deferredTargetUri: null });
    }
    return out;
  }

  const dir = extractPath(await open({ directory: true }));
  if (!dir) return null;

  const claimed = new Set<string>();
  const out: ResolvedReceivePath[] = [];
  for (const filename of filenames) {
    out.push({
      backendPath: await uniqueReceivePath(dir, filename, claimed),
      deferredTargetUri: null,
    });
  }
  return out;
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
    const tempPath = await writeBytesToAppCache(cleaned, filename, 'send');
    return { backendPath: tempPath, filename };
  } catch {
    return { backendPath: localPath, filename };
  }
}

export async function prepareOfferSourcePath(rawSelection: string): Promise<PreparedFile> {
  const filename = filenameFromPath(rawSelection);

  if (rawSelection.startsWith('file://')) {
    return await offerWithoutMetadata(pathFromFileUri(rawSelection), filename);
  }

  if (rawSelection.startsWith('content://') || isUriPath(rawSelection)) {
    const bytes = await readFile(rawSelection);
    const tempPath = await writeBytesToAppCache(stripImageMetadata(bytes), filename, 'send');
    return { backendPath: tempPath, filename };
  }

  return await offerWithoutMetadata(rawSelection, filename);
}

export async function resolveReceiveSavePath(
  rawSelection: string,
  defaultFilename: string
): Promise<ResolvedReceivePath> {
  if (rawSelection.startsWith('file://')) {
    return {
      backendPath: pathFromFileUri(rawSelection),
      deferredTargetUri: null,
    };
  }

  if (rawSelection.startsWith('content://')) {
    const backendPath = await writeBytesToAppCache(new Uint8Array(0), defaultFilename, 'recv');
    return { backendPath, deferredTargetUri: rawSelection };
  }

  return { backendPath: rawSelection, deferredTargetUri: null };
}

export function registerDeferredReceiveTarget(
  backendPath: string,
  targetUri: string,
  filename: string
) {
  deferredDownloadTargets.set(backendPath, { targetUri, filename });
}

export async function finalizeDeferredReceiveTarget(savePath: string): Promise<{
  filename: string;
  moved: boolean;
}> {
  const deferred = deferredDownloadTargets.get(savePath);
  if (!deferred) {
    return { filename: filenameFromPath(savePath), moved: false };
  }

  try {
    try {
      await copyFile(savePath, deferred.targetUri);
    } catch {
      const bytes = await readFile(savePath);
      await writeFile(deferred.targetUri, bytes);
    }
    await remove(savePath);
    deferredDownloadTargets.delete(savePath);
    return { filename: deferred.filename, moved: true };
  } catch {
    deferredDownloadTargets.delete(savePath);
    throw new Error('Could not finalize downloaded file to selected destination');
  }
}
