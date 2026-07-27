import raw from '../../../CHANGELOG.md?raw';

export interface ChangelogEntry {
  version: string;
  date: string | null;
  items: string[];
}

function cleanItem(line: string): string {
  return line
    .replace(/^-\s+/, '')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/\*\*([^*]+)\*\*/g, '$1')
    .replace(/`([^`]+)`/g, '$1')
    .trim();
}

export function parseChangelog(md: string): ChangelogEntry[] {
  const entries: ChangelogEntry[] = [];
  let current: ChangelogEntry | null = null;
  for (const line of md.split('\n')) {
    const heading = line.match(/^##\s+\[?([^\]\s]+)\]?(?:\s*-\s*(\S+))?\s*$/);
    if (heading) {
      current = null;
      if (heading[1].toLowerCase() !== 'unreleased') {
        current = { version: heading[1], date: heading[2] ?? null, items: [] };
        entries.push(current);
      }
      continue;
    }
    if (current && /^-\s+/.test(line)) {
      const item = cleanItem(line);
      if (item) current.items.push(item);
    }
  }
  return entries.filter((e) => e.items.length > 0);
}

const parsed = parseChangelog(raw);

export function latestEntry(): ChangelogEntry | null {
  return parsed[0] ?? null;
}

export function olderEntries(): ChangelogEntry[] {
  return parsed.slice(1);
}
