import { marked, type Tokens } from 'marked';
import DOMPurify from 'dompurify';
import { convertFileSrc } from '@tauri-apps/api/core';
import { openUrl } from '@tauri-apps/plugin-opener';
import { notifications } from '$lib/state/notifications.svelte';
import { notifyError } from '$lib/utils/errors';
import { confirmDialogWithCheckbox } from '$lib/state/confirm.svelte';
import { trustedDomainsState } from '$lib/state/trustedDomains.svelte';
import { clockOptions } from '$lib/utils/timeFormat';
import { t, dateLocale } from '$lib/i18n';

marked.use({
  extensions: [
    {
      name: 'spoiler',
      level: 'inline',
      start(src: string) {
        const i = src.indexOf('||');
        return i < 0 ? undefined : i;
      },
      tokenizer(this: { lexer: { inlineTokens: (s: string) => Tokens.Generic[] } }, src: string) {
        const match = /^\|\|([\s\S]+?)\|\|/.exec(src);
        if (match) {
          return {
            type: 'spoiler',
            raw: match[0],
            tokens: this.lexer.inlineTokens(match[1]),
          };
        }
      },
      renderer(
        this: { parser: { parseInline: (tokens: Tokens.Generic[]) => string } },
        token: Tokens.Generic
      ) {
        return `<span class="spoiler" tabindex="0">${this.parser.parseInline(token.tokens ?? [])}</span>`;
      },
    },
  ],
});

function b64EncodeUtf8(text: string): string {
  const bytes = new TextEncoder().encode(text);
  let binary = '';
  for (let i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
  return btoa(binary);
}

function b64DecodeUtf8(encoded: string): string {
  const binary = atob(encoded);
  const bytes = Uint8Array.from(binary, (c) => c.charCodeAt(0));
  return new TextDecoder().decode(bytes);
}

marked.use({
  renderer: {
    code({ text, lang }: { text: string; lang?: string }) {
      const encoded = b64EncodeUtf8(text);
      const escapedCode = text.replace(/</g, '&lt;').replace(/>/g, '&gt;');
      const aria = t('chat.bubble.copyCodeAria');
      const langAttr = lang ? ` class="language-${lang}"` : '';
      return `<div class="code-wrap"><button class="code-copy" data-code="${encoded}" aria-label="${aria}">⧉</button><pre><code${langAttr}>${escapedCode}</code></pre></div>`;
    },
  },
});

export type MediaKind = 'image' | 'audio' | 'video' | 'other';

const IMAGE_EXT = new Set([
  'png',
  'jpg',
  'jpeg',
  'gif',
  'webp',
  'bmp',
  'svg',
  'avif',
  'heic',
  'heif',
]);
const AUDIO_EXT = new Set(['mp3', 'wav', 'ogg', 'oga', 'm4a', 'aac', 'flac', 'opus', 'weba']);
const VIDEO_EXT = new Set(['mp4', 'webm', 'mov', 'm4v', 'ogv', 'mkv', 'avi']);

export function mediaKindFromFilename(filename: string): MediaKind {
  const dot = filename.lastIndexOf('.');
  if (dot < 0) return 'other';
  const ext = filename.slice(dot + 1).toLowerCase();
  if (IMAGE_EXT.has(ext)) return 'image';
  if (AUDIO_EXT.has(ext)) return 'audio';
  if (VIDEO_EXT.has(ext)) return 'video';
  return 'other';
}

const TEXT_EXT = new Set([
  'txt',
  'md',
  'markdown',
  'log',
  'json',
  'yaml',
  'yml',
  'toml',
  'diff',
  'patch',
]);

export function isTextFilename(filename: string): boolean {
  const dot = filename.lastIndexOf('.');
  if (dot < 0) return false;
  return TEXT_EXT.has(filename.slice(dot + 1).toLowerCase());
}

export function midTruncate(name: string, maxLen = 30): string {
  if (name.length <= maxLen) return name;
  const dot = name.lastIndexOf('.');
  const ext = dot > 0 && name.length - dot < 8 ? name.slice(dot) : '';
  const stem = ext ? name.slice(0, name.length - ext.length) : name;
  const room = maxLen - ext.length - 1;
  if (room < 6) return name.slice(0, maxLen - 1) + '…';
  const head = Math.ceil(room * 0.6);
  const tail = room - head;
  return stem.slice(0, head) + '…' + stem.slice(stem.length - tail) + ext;
}

export function fileTypeColor(filename: string): string {
  const dot = filename.lastIndexOf('.');
  if (dot < 0) return 'var(--text-muted)';
  const ext = filename.slice(dot + 1).toLowerCase();
  if (ext === 'pdf') return '#ef4444';
  if (['doc', 'docx', 'odt', 'rtf'].includes(ext)) return '#3b82f6';
  if (['xls', 'xlsx', 'csv', 'ods'].includes(ext)) return '#22c55e';
  if (['ppt', 'pptx', 'odp', 'key'].includes(ext)) return '#f97316';
  if (['zip', 'tar', 'gz', '7z', 'rar', 'bz2', 'xz'].includes(ext)) return '#a855f7';
  if (AUDIO_EXT.has(ext)) return '#ec4899';
  if (VIDEO_EXT.has(ext)) return '#14b8a6';
  if (IMAGE_EXT.has(ext)) return '#6366f1';
  if (['txt', 'md', 'log'].includes(ext)) return '#94a3b8';
  if (['json', 'xml', 'yaml', 'yml', 'toml'].includes(ext)) return '#eab308';
  if (
    [
      'js',
      'ts',
      'tsx',
      'jsx',
      'rs',
      'py',
      'go',
      'rb',
      'java',
      'c',
      'cpp',
      'h',
      'hpp',
      'cs',
      'swift',
      'kt',
      'php',
      'html',
      'css',
      'scss',
    ].includes(ext)
  )
    return '#0ea5e9';
  return 'var(--text-muted)';
}

export function formatFileSize(bytes: number): string {
  if (bytes <= 0) return '';
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

export interface CallRecord {
  outcome: string;
  durationMs: number;
}

function fmtCallDuration(ms: number): string {
  const s = Math.round(ms / 1000);
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`;
}

export function isMissedCall(rec: CallRecord): boolean {
  return rec.outcome === 'missed' || rec.outcome === 'declined';
}

type TransferProgress = { bytesTransferred: number; totalBytes: number } | null | undefined;

export function transferPercent(p: TransferProgress): number {
  if (!p || p.totalBytes <= 0) return 0;
  return Math.max(0, Math.min(100, Math.round((p.bytesTransferred / p.totalBytes) * 100)));
}

export function isTransferDone(p: TransferProgress): boolean {
  if (!p || p.totalBytes <= 0) return false;
  return p.bytesTransferred >= p.totalBytes;
}

// Local media URL for a received file. `version` (bumped on `file_received`) busts
// the webview cache, since the element mounts while the destination is still the
// empty preallocated file. The asset protocol routes on the path only, so the query is inert.
export function mediaUrl(path: string, version: number): string {
  const base = convertFileSrc(path);
  return version > 0 ? `${base}?v=${version}` : base;
}

// Places the emoji picker relative to its anchor, preferring above, then below, then
// clamped into the viewport. PICKER_H is the *maximum* height (search shrinks it), so
// the result pins the edge facing the anchor, letting the picker shrink without detaching.
export const PICKER_W = 288;
const PICKER_H = 248;
export interface EmojiPickerPos {
  top: number | null;
  bottom: number | null;
  left: number;
}
export function emojiPickerPosition(anchor: DOMRect, vw: number, vh: number): EmojiPickerPos {
  const margin = 8;
  const spaceAbove = anchor.top;
  const spaceBelow = vh - anchor.bottom;
  let top: number | null = null;
  let bottom: number | null = null;
  if (spaceAbove >= PICKER_H + margin) {
    bottom = vh - anchor.top + margin;
  } else if (spaceBelow >= PICKER_H + margin) {
    top = anchor.bottom + margin;
  } else {
    // Neither side fits: pin to the bottom of the viewport, which keeps the
    // picker's growth/shrink anchored the same way as the "above" case.
    bottom = margin;
  }
  let left = anchor.left + anchor.width / 2 - PICKER_W / 2;
  left = Math.max(margin, Math.min(left, vw - PICKER_W - margin));
  return { top, bottom, left };
}

export function callRecordLabel(rec: CallRecord, direction: string): string {
  switch (rec.outcome) {
    case 'started':
      return t('chat.call.recordStarted');
    case 'completed':
      return t('chat.call.recordCall', { duration: fmtCallDuration(rec.durationMs) });
    case 'missed':
      return direction === 'received' ? t('chat.call.missed') : t('chat.call.noAnswer');
    case 'declined':
      return t('chat.call.declined');
    case 'busy':
      return t('chat.call.recordBusy');
    case 'canceled':
      return t('chat.call.recordCanceled');
    default:
      return t('chat.call.recordEnded');
  }
}

export function flatStatusLabel(status: string): string {
  if (status === 'sending') return t('chat.bubble.statusSending');
  if (status === 'queued') return t('chat.bubble.statusQueued');
  if (status === 'queued_in_dht') return t('chat.bubble.statusUploaded');
  if (status === 'delivered') return t('chat.bubble.statusSent');
  if (status === 'offline_delivered') return t('chat.bubble.statusSentOffline');
  if (status === 'read') return t('chat.bubble.statusRead');
  if (status === 'failed') return t('chat.bubble.statusFailed');
  return '';
}

export function isMessageActionable(status: string): boolean {
  return status !== 'sending' && status !== 'failed' && status !== 'queued';
}

export function getMessagePreview(content: string): string {
  const clean = content.replace(/\s+/g, ' ').trim();
  if (!clean) return t('chat.bubble.emptyPreview');
  return clean.length > 80 ? clean.slice(0, 77) + '...' : clean;
}

export function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString(dateLocale(), clockOptions());
}

export function formatGroupTime(ts: number): string {
  const d = new Date(ts);
  const now = new Date();
  const isToday = d.toDateString() === now.toDateString();
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  const isYesterday = d.toDateString() === yesterday.toDateString();

  if (isToday) return `${t('time.today')} · ${formatTime(ts)}`;
  if (isYesterday) return `${t('time.yesterday')} · ${formatTime(ts)}`;
  if (now.getTime() - ts < 7 * 24 * 3600 * 1000) {
    return d.toLocaleDateString(dateLocale(), { weekday: 'long' }) + ` · ${formatTime(ts)}`;
  }
  return (
    d.toLocaleDateString(dateLocale(), {
      month: 'short',
      day: 'numeric',
      year: d.getFullYear() !== now.getFullYear() ? 'numeric' : undefined,
    }) + ` · ${formatTime(ts)}`
  );
}

export function isSameDay(a: number, b: number): boolean {
  return new Date(a).toDateString() === new Date(b).toDateString();
}

export function formatDaySeparator(ts: number): string {
  const d = new Date(ts);
  const now = new Date();
  if (d.toDateString() === now.toDateString()) return t('time.today');
  const yesterday = new Date(now);
  yesterday.setDate(yesterday.getDate() - 1);
  if (d.toDateString() === yesterday.toDateString()) return t('time.yesterday');
  if (now.getTime() - ts < 7 * 24 * 3600 * 1000) {
    return d.toLocaleDateString(dateLocale(), { weekday: 'long' });
  }
  return d.toLocaleDateString(dateLocale(), {
    month: 'short',
    day: 'numeric',
    year: d.getFullYear() !== now.getFullYear() ? 'numeric' : undefined,
  });
}

const EMOJI_RE = /^(\p{Emoji_Presentation}|\p{Extended_Pictographic})(‍|️|\p{Emoji_Modifier})*$/u;
const seg = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

export function isEmojiOnly(content: string): { jumbo: boolean; count: number } {
  const trimmed = content.trim();
  if (!trimmed) return { jumbo: false, count: 0 };
  let count = 0;
  for (const { segment } of seg.segment(trimmed)) {
    if (segment.trim() === '') continue;
    if (!EMOJI_RE.test(segment)) return { jumbo: false, count: 0 };
    count++;
    if (count > 3) return { jumbo: false, count };
  }
  return { jumbo: count >= 1 && count <= 3, count };
}

const markdownCache = new Map<string, string>();

// Wraps occurrences of `term` in <mark> within the text of already-sanitized
// HTML (skips anything inside tags, and treats HTML entities like &amp; as
// atomic so a term such as "amp" can't corrupt them).
const HTML_ENTITY_RE = /&(?:[a-zA-Z][a-zA-Z0-9]*|#\d+|#x[0-9a-fA-F]+);/g;

function highlightPlainText(text: string, re: RegExp): string {
  let out = '';
  let last = 0;
  for (const m of text.matchAll(HTML_ENTITY_RE)) {
    out += text.slice(last, m.index).replace(re, '<mark>$1</mark>') + m[0];
    last = m.index + m[0].length;
  }
  return out + text.slice(last).replace(re, '<mark>$1</mark>');
}

export function highlightTerm(html: string, term: string): string {
  const t = term.trim();
  if (!t) return html;
  const esc = t.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const re = new RegExp(`(${esc})`, 'gi');
  return html.replace(/(<[^>]+>)|([^<]+)/g, (_m, tag, text) =>
    tag ? tag : highlightPlainText(text as string, re)
  );
}

export function renderMarkdown(content: string, isEdited: boolean = false): string {
  const cacheKey = content + (isEdited ? '|e' : '|n');
  const cached = markdownCache.get(cacheKey);
  if (cached) return cached;

  if (isEmojiOnly(content).jumbo) {
    const escaped = content.trim().replace(/</g, '&lt;').replace(/>/g, '&gt;');
    const sanitized = DOMPurify.sanitize(`<p class="jumbo-emoji">${escaped}</p>`, {
      ALLOWED_TAGS: ['p'],
      ALLOWED_ATTR: ['class'],
    });
    markdownCache.set(cacheKey, sanitized);
    if (markdownCache.size > 600) {
      const firstKey = markdownCache.keys().next().value;
      if (firstKey) markdownCache.delete(firstKey);
    }
    return sanitized;
  }

  const escaped = content.replace(/</g, '&lt;').replace(/>/g, '&gt;');
  let html = marked.parse(escaped, {
    async: false,
    gfm: true,
    breaks: true,
  }) as string;

  if (isEdited) {
    const badge = ' <span class="edited-tag">(edited)</span>';
    if (html.endsWith('</p>\n')) html = html.replace(/<\/p>\n$/, `${badge}</p>\n`);
    else if (html.endsWith('</p>')) html = html.replace(/<\/p>$/, `${badge}</p>`);
    else html += badge;
  }

  const sanitized = DOMPurify.sanitize(html, {
    ALLOWED_TAGS: [
      'p',
      'br',
      'b',
      'i',
      'em',
      'strong',
      'a',
      'pre',
      'code',
      'blockquote',
      'ul',
      'ol',
      'li',
      'del',
      'h1',
      'h2',
      'h3',
      'h4',
      'h5',
      'h6',
      'hr',
      'span',
      'div',
      'button',
    ],
    ALLOWED_ATTR: [
      'href',
      'class',
      'target',
      'rel',
      'style',
      'tabindex',
      'data-code',
      'aria-label',
    ],
  });
  markdownCache.set(cacheKey, sanitized);
  if (markdownCache.size > 600) {
    const firstKey = markdownCache.keys().next().value;
    if (firstKey) markdownCache.delete(firstKey);
  }
  return sanitized;
}

export function formatFullTimestamp(ts: number): string {
  return new Date(ts).toLocaleString(dateLocale(), {
    year: 'numeric',
    month: 'short',
    day: 'numeric',
    ...clockOptions(),
  });
}

// Messages carry a sent time (from the MessageId) and a received time. Only when
// the gap is meaningful (delayed offline delivery) do we surface "Received …".
export const DELAYED_RECEIVE_HOVER_THRESHOLD_MS = 60_000;

export function receivedHoverLabel(msg: {
  direction: string;
  timestamp: number;
  receivedTimestamp: number;
}): string | null {
  if (msg.direction !== 'received') return null;
  if (msg.receivedTimestamp - msg.timestamp < DELAYED_RECEIVE_HOVER_THRESHOLD_MS) return null;
  return formatFullTimestamp(msg.receivedTimestamp);
}

export async function handleMarkdownClick(e: MouseEvent) {
  const target = e.target as HTMLElement | null;
  const copyBtn = target?.closest('button.code-copy') as HTMLElement | null;
  if (copyBtn) {
    e.preventDefault();
    const enc = copyBtn.getAttribute('data-code') ?? '';
    try {
      const text = b64DecodeUtf8(enc);
      await navigator.clipboard.writeText(text);
      copyBtn.classList.add('copied');
      setTimeout(() => copyBtn.classList.remove('copied'), 1200);
    } catch {}
    return;
  }
  const spoiler = target?.closest('.spoiler');
  if (spoiler) {
    e.preventDefault();
    e.stopPropagation();
    spoiler.classList.toggle('revealed');
    return;
  }
  const anchor = target?.closest('a') as HTMLAnchorElement | null;
  if (!anchor) return;
  e.preventDefault();
  e.stopPropagation();
  const href = anchor.getAttribute('href');
  if (!href) return;
  try {
    const url = new URL(href, window.location.origin);
    if (!['http:', 'https:', 'mailto:', 'tel:'].includes(url.protocol)) {
      notifications.push(t('chat.bubble.linkConfirm.unsupported'), 'error');
      return;
    }
    const isWeb = url.protocol === 'http:' || url.protocol === 'https:';
    if (isWeb && !trustedDomainsState.isTrusted(url.hostname)) {
      const result = await confirmDialogWithCheckbox({
        title: t('chat.bubble.linkConfirm.title'),
        message: t('chat.bubble.linkConfirm.message'),
        detail: url.toString(),
        confirmLabel: t('chat.bubble.linkConfirm.open'),
        cancelLabel: t('chat.bubble.linkConfirm.cancel'),
        tone: 'warning',
        checkbox: {
          label: t('chat.bubble.linkConfirm.trustDomain', { host: url.hostname }),
        },
      });
      if (!result.confirmed) return;
      if (result.checked) trustedDomainsState.trust(url.hostname);
    }
    await openUrl(url.toString());
  } catch (err) {
    notifyError(err, 'chat.bubble.linkConfirm.error');
  }
}
