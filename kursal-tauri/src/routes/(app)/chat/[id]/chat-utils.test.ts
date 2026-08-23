import { describe, it, expect, vi } from 'vitest';

// chat-utils pulls Tauri/state/i18n deps for its click handler; stub them so
// the pure rendering/formatting helpers can be tested in isolation. marked and
// DOMPurify are intentionally NOT mocked - sanitization is what we verify.
vi.mock('@tauri-apps/plugin-opener', () => ({ openUrl: vi.fn() }));
vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (p: string) => `asset://localhost/${encodeURIComponent(p)}`,
}));
vi.mock('$lib/state/notifications.svelte', () => ({
  notifications: { push: vi.fn() },
}));
vi.mock('$lib/state/confirm.svelte', () => ({
  confirmDialogWithCheckbox: vi.fn(),
}));
vi.mock('$lib/state/trustedDomains.svelte', () => ({
  trustedDomainsState: { isTrusted: () => true, trust: vi.fn() },
}));
vi.mock('$lib/i18n', () => ({ t: (k: string) => k, dateLocale: () => 'en' }));

const {
  renderMarkdown,
  mediaKindFromFilename,
  getMessagePreview,
  isEmojiOnly,
  isMissedCall,
  transferPercent,
  isTransferDone,
  emojiPickerPosition,
  receivedHoverLabel,
  isMessageActionable,
} = await import('./chat-utils');

const { formatFileSize } = await import('$lib/utils/bytes');
const { midTruncate } = await import('$lib/utils/text');
const { formatFullTimestamp } = await import('$lib/utils/dateFormat.svelte');

describe('isMessageActionable', () => {
  it('blocks messages that have not reached the peer', () => {
    for (const s of ['sending', 'queued', 'failed']) {
      expect(isMessageActionable(s)).toBe(false);
    }
  });
  it('allows stored and delivered messages', () => {
    for (const s of ['queued_in_dht', 'delivered', 'offline_delivered', 'read']) {
      expect(isMessageActionable(s)).toBe(true);
    }
  });
});

describe('receivedHoverLabel', () => {
  it('returns null for a small sent/received gap', () => {
    expect(
      receivedHoverLabel({ direction: 'received', timestamp: 0, receivedTimestamp: 30_000 })
    ).toBeNull();
  });
  it('returns a label for a delayed received message', () => {
    expect(
      receivedHoverLabel({ direction: 'received', timestamp: 0, receivedTimestamp: 120_000 })
    ).not.toBeNull();
  });
  it('returns null for sent messages regardless of gap', () => {
    expect(
      receivedHoverLabel({ direction: 'sent', timestamp: 0, receivedTimestamp: 120_000 })
    ).toBeNull();
  });
});

describe('renderMarkdown (sanitization)', () => {
  it('renders basic markdown', () => {
    const out = renderMarkdown('**bold**');
    expect(out).toContain('<strong>bold</strong>');
  });

  it('escapes raw <script> so no executable tag survives', () => {
    const out = renderMarkdown('<script>alert(1)</script>');
    expect(out).not.toContain('<script');
    expect(out).toContain('&lt;script&gt;');
  });

  it('neutralizes raw HTML img injection (rendered as text, not a tag)', () => {
    const out = renderMarkdown('<img src=x onerror=alert(1)>');
    expect(out).not.toContain('<img');
    expect(out).toContain('&lt;img');
  });

  it('strips javascript: links', () => {
    const out = renderMarkdown('[x](javascript:alert(1))');
    expect(out).not.toContain('javascript:');
  });

  it('keeps safe https links', () => {
    const out = renderMarkdown('[k](https://kursal.chat)');
    expect(out).toContain('href="https://kursal.chat"');
  });

  it('appends an edited badge when isEdited', () => {
    const out = renderMarkdown('hi', true);
    expect(out).toContain('edited-tag');
  });
});

describe('pure helpers', () => {
  it('classifies media by extension', () => {
    expect(mediaKindFromFilename('a.png')).toBe('image');
    expect(mediaKindFromFilename('a.mp3')).toBe('audio');
    expect(mediaKindFromFilename('a.mp4')).toBe('video');
    expect(mediaKindFromFilename('a.txt')).toBe('other');
    expect(mediaKindFromFilename('noext')).toBe('other');
  });

  it('formats file sizes', () => {
    expect(formatFileSize(0)).toBe('');
    expect(formatFileSize(512)).toBe('512 B');
    expect(formatFileSize(2048)).toBe('2.0 KB');
  });

  it('previews and truncates message content', () => {
    expect(getMessagePreview('   ')).toBe('chat.bubble.emptyPreview');
    expect(getMessagePreview('hello   world')).toBe('hello world');
    expect(getMessagePreview('x'.repeat(100)).endsWith('…')).toBe(true);
  });

  it('mid-truncates long filenames keeping the extension', () => {
    const out = midTruncate('a-really-long-file-name-here.pdf', 20);
    expect(out.length).toBeLessThanOrEqual(20);
    expect(out).toContain('…');
    expect(out.endsWith('.pdf')).toBe(true);
  });
});

describe('isEmojiOnly', () => {
  it('flags 1-3 bare emoji', () => {
    expect(isEmojiOnly('😀').jumbo).toBe(true);
    expect(isEmojiOnly('😀🎉🔥').jumbo).toBe(true);
    expect(isEmojiOnly('👋🏽').jumbo).toBe(true); // toned counts as one
  });
  it('rejects text + emoji', () => {
    expect(isEmojiOnly('hi 😀').jumbo).toBe(false);
  });
  it('rejects 4+ emoji', () => {
    expect(isEmojiOnly('😀😀😀😀').jumbo).toBe(false);
  });
  it('rejects empty', () => {
    expect(isEmojiOnly('   ').jumbo).toBe(false);
  });
});

describe('formatFullTimestamp', () => {
  it('formats an absolute date-time string', () => {
    const s = formatFullTimestamp(new Date(2026, 5, 21, 14, 32).getTime());
    expect(s).toMatch(/2026/);
    expect(s).toMatch(/14|2:32|32/);
  });
});

describe('isMissedCall', () => {
  it('flags missed and declined as missed', () => {
    expect(isMissedCall({ outcome: 'missed', durationMs: 0 })).toBe(true);
    expect(isMissedCall({ outcome: 'declined', durationMs: 0 })).toBe(true);
  });
  it('does not flag completed or started calls', () => {
    expect(isMissedCall({ outcome: 'completed', durationMs: 5000 })).toBe(false);
    expect(isMissedCall({ outcome: 'started', durationMs: 0 })).toBe(false);
  });
});

describe('transfer progress', () => {
  it('returns 0% / not-done for missing or zero-total progress', () => {
    for (const p of [null, undefined, { bytesTransferred: 5, totalBytes: 0 }]) {
      expect(transferPercent(p)).toBe(0);
      expect(isTransferDone(p)).toBe(false);
    }
  });
  it('computes a clamped, rounded percentage', () => {
    expect(transferPercent({ bytesTransferred: 50, totalBytes: 200 })).toBe(25);
    expect(transferPercent({ bytesTransferred: 999, totalBytes: 200 })).toBe(100);
  });
  it('marks done only when fully transferred', () => {
    expect(isTransferDone({ bytesTransferred: 199, totalBytes: 200 })).toBe(false);
    expect(isTransferDone({ bytesTransferred: 200, totalBytes: 200 })).toBe(true);
  });
});

describe('emojiPickerPosition', () => {
  const rect = (over: Partial<DOMRect>): DOMRect => ({
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    width: 0,
    height: 0,
    x: 0,
    y: 0,
    toJSON: () => ({}),
    ...over,
  });

  it('pins the bottom edge to the anchor when placed above', () => {
    const pos = emojiPickerPosition(
      rect({ top: 400, bottom: 430, left: 500, width: 40 }),
      1000,
      800
    );
    // Bottom-anchored so shrinking on search keeps the picker against the
    // message instead of leaving it floating.
    expect(pos.top).toBeNull();
    expect(pos.bottom).toBe(800 - 400 + 8);
  });

  it('falls below when there is no room above', () => {
    const pos = emojiPickerPosition(rect({ top: 10, bottom: 40, left: 500, width: 40 }), 1000, 800);
    expect(pos.bottom).toBeNull();
    expect(pos.top).toBe(40 + 8);
  });

  it('pins to the viewport bottom when neither side fits', () => {
    const pos = emojiPickerPosition(
      rect({ top: 120, bottom: 150, left: 500, width: 40 }),
      1000,
      300
    );
    expect(pos.top).toBeNull();
    expect(pos.bottom).toBe(8);
  });

  it('clamps horizontally into the viewport', () => {
    const pos = emojiPickerPosition(
      rect({ top: 400, bottom: 430, left: 990, width: 10 }),
      1000,
      800
    );
    expect(pos.left).toBe(1000 - 288 - 8);
    expect(pos.left).toBeGreaterThanOrEqual(8);
  });
});
