import { describe, expect, it } from 'vitest';
import { parseChangelog, parseReleaseNotes } from './changelog';

const SAMPLE = `# Changelog

Intro text with a [link](https://example.com).

## [Unreleased]

- Should be ignored

## [1.2.0] - 2026-08-01

### Added

- **Bold** feature with \`code\`
- See [docs](https://example.com) for details

### Fixed

- A bug

## 1.1.0

- Bare heading style

## [1.0.0] - 2026-01-01
`;

describe('parseChangelog', () => {
  const entries = parseChangelog(SAMPLE);

  it('skips Unreleased and empty sections', () => {
    expect(entries.map((e) => e.version)).toEqual(['1.2.0', '1.1.0']);
  });

  it('splits sections into features and fixes and strips markdown', () => {
    expect(entries[0].groups).toEqual([
      { kind: 'features', items: ['Bold feature with code', 'See docs for details'] },
      { kind: 'fixes', items: ['A bug'] },
    ]);
  });

  it('files items without a section under other', () => {
    expect(entries[1].groups).toEqual([{ kind: 'other', items: ['Bare heading style'] }]);
  });

  it('parses date and bare headings', () => {
    expect(entries[0].date).toBe('2026-08-01');
    expect(entries[1].date).toBeNull();
  });

  it('parses the real repo CHANGELOG shape', () => {
    const real = parseChangelog(
      '## [Unreleased]\n\n## [0.1.0-beta] - 2026-07-17\n\n### Features\n\n- Video calls\n\n### Bug Fixes\n\n- Audio jitter\n\n### Miscellaneous\n\n- Clippy\n'
    );
    expect(real[0].version).toBe('0.1.0-beta');
    expect(real[0].groups.map((g) => g.kind)).toEqual(['features', 'fixes', 'other']);
    expect(real[0].groups[0].items).toEqual(['Video calls']);
  });
});

describe('parseReleaseNotes', () => {
  it('groups a git-cliff release body', () => {
    const groups = parseReleaseNotes(
      '### Bug Fixes\n\n- Otp consumption\n\n### Features\n\n- Log view\n'
    );
    expect(groups).toEqual([
      { kind: 'features', items: ['Log view'] },
      { kind: 'fixes', items: ['Otp consumption'] },
    ]);
  });

  it('returns nothing for prose without bullets', () => {
    expect(parseReleaseNotes('Just a plain release description.')).toEqual([]);
  });
});
