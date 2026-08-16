import { copyFile, readFile, writeFile } from '@tauri-apps/plugin-fs';
import { open, save } from '@tauri-apps/plugin-dialog';
import { availableSpace, createOutgoingPendingPath, resolveDownloadPath } from '$lib/api/messages';
import { isMobile } from '$lib/api/window';

export const LARGE_FILE_PROMPT_BYTES = 5 * 1024 * 1024 * 1024;

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
 * Hands bytes the webview holds to a core-owned staging slot and returns its path.
 * The filename survives untouched; uniqueness lives in the directory the core
 * picks, never in the filename itself.
 */
async function stagePendingBytes(bytes: Uint8Array, filename: string): Promise<string> {
  const path = await createOutgoingPendingPath(filename);
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

/** Which native picker mobile should open; desktop ignores it. */
export type PickerMode = 'media' | 'document';

/**
 * Desktop file picker, and the iOS media/document pickers. Android instead picks
 * through the webview's own file inputs (see AttachSheet): its dialog plugin hands
 * back opaque content:// ids with no real filename, while iOS returns real paths.
 */
export async function pickFilesForSend(pickerMode?: PickerMode): Promise<PreparedFile[]> {
  const selected = await open({ multiple: true, directory: false, pickerMode });
  const raws = extractPaths(selected);
  return await Promise.all(raws.map((r) => prepareOfferSourcePath(r)));
}

/**
 * Desktop picker selections and OS drag-and-drop payloads. The file is offered
 * straight from where it already lives: nothing is read or copied here, so a
 * multi-gigabyte drop costs nothing. `send_file_offer` streams and copies it later.
 */
export async function prepareOfferSourcePath(rawSelection: string): Promise<PreparedFile> {
  const filename = filenameFromPath(rawSelection);
  const backendPath = rawSelection.startsWith('file://')
    ? pathFromFileUri(rawSelection)
    : rawSelection;
  return { backendPath, filename };
}

/** Webview File objects: mobile pickers and the camera input. */
export async function prepareOfferFromFile(
  file: File,
  fallbackName: string
): Promise<PreparedFile> {
  const filename = sanitizeFilename(file.name || fallbackName);
  const bytes = new Uint8Array(await file.arrayBuffer());
  return { backendPath: await stagePendingBytes(bytes, filename), filename };
}

/** Pasted image bytes, which arrive without any name of their own. */
export async function prepareOfferFromBytes(
  bytes: Uint8Array,
  filename: string
): Promise<PreparedFile> {
  return { backendPath: await stagePendingBytes(bytes, filename), filename };
}

export function needsDestinationPrompt(sizeBytes: number, freeBytes: number | null): boolean {
  if (sizeBytes > LARGE_FILE_PROMPT_BYTES) return true;
  return freeBytes !== null && sizeBytes > freeBytes;
}

/**
 * Destination for a manually accepted file. Returns null when the picker is
 * dismissed, which cancels the accept.
 */
export async function resolveAcceptDestination(
  contactId: string,
  offerId: string,
  filename: string,
  sizeBytes: number
): Promise<string | null> {
  const fallback = await resolveDownloadPath(contactId, offerId, filename);
  if (isMobile) return fallback;

  const free = await availableSpace(fallback).catch(() => null);
  if (!needsDestinationPrompt(sizeBytes, free)) return fallback;

  const target = extractPath(await save({ defaultPath: sanitizeFilename(filename) }));
  return target || null;
}

/**
 * Copies an already-downloaded file out of the app's storage to wherever the user
 * wants it: the explicit "get it out of the app" action, and the only way off
 * mobile since app storage isn't browsable there. Returns false if the picker is dismissed.
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
