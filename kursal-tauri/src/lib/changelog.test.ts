import { describe, expect, it } from 'vitest';
import { parseChangelog } from './changelog';

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

  it('collects items across subsections and strips markdown', () => {
    expect(entries[0].items).toEqual(['Bold feature with code', 'See docs for details', 'A bug']);
  });

  it('parses date and bare headings', () => {
    expect(entries[0].date).toBe('2026-08-01');
    expect(entries[1].date).toBeNull();
  });

  it('parses the real repo CHANGELOG shape', () => {
    const real = parseChangelog(
      '## [Unreleased]\n\n## [0.1.0-beta] - 2026-07-17\n\n### Added\n\n- Video calls\n'
    );
    expect(real[0].version).toBe('0.1.0-beta');
    expect(real[0].items).toContain('Video calls');
  });
});
