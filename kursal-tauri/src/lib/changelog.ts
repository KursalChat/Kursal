import raw from '../../../CHANGELOG.md?raw';
import { t } from '$lib/i18n';

export type ChangeKind = 'features' | 'fixes' | 'other';

export interface ChangeGroup {
  kind: ChangeKind;
  items: string[];
}

export interface ChangelogEntry {
  version: string;
  date: string | null;
  groups: ChangeGroup[];
}

const KIND_ORDER: ChangeKind[] = ['features', 'fixes', 'other'];

export function groupLabel(kind: ChangeKind): string {
  if (kind === 'features') return t('changelog.features');
  if (kind === 'fixes') return t('changelog.fixes');
  return t('changelog.other');
}

function cleanItem(line: string): string {
  return line
    .replace(/^[-*]\s+/, '')
    .replace(/\[([^\]]+)\]\([^)]*\)/g, '$1')
    .replace(/\*\*([^*]+)\*\*/g, '$1')
    .replace(/`([^`]+)`/g, '$1')
    .trim();
}

const HIDDEN = new Set(['testing', 'build', 'ci', 'styling', 'revert']);

function kindOf(heading: string): ChangeKind | null {
  const h = heading.toLowerCase();
  if (HIDDEN.has(h)) return null;
  if (h.includes('fix')) return 'fixes';
  if (h.includes('feature') || h.includes('added') || h.includes('performance')) return 'features';
  return 'other';
}

type Buckets = Map<ChangeKind, string[]>;

function addItem(buckets: Buckets, kind: ChangeKind, item: string) {
  const list = buckets.get(kind);
  if (list) list.push(item);
  else buckets.set(kind, [item]);
}

function toGroups(buckets: Buckets): ChangeGroup[] {
  return KIND_ORDER.filter((k) => buckets.get(k)?.length).map((k) => ({
    kind: k,
    items: buckets.get(k)!,
  }));
}

export function parseReleaseNotes(md: string): ChangeGroup[] {
  const buckets: Buckets = new Map();
  let kind: ChangeKind | null = 'other';
  for (const line of md.split('\n')) {
    const heading = line.match(/^#{1,6}\s+(.+?)\s*$/);
    if (heading) {
      kind = kindOf(heading[1]);
      continue;
    }
    if (kind && /^[-*]\s+/.test(line)) {
      const item = cleanItem(line);
      if (item) addItem(buckets, kind, item);
    }
  }
  return toGroups(buckets);
}

export function parseChangelog(md: string): ChangelogEntry[] {
  const entries: { entry: ChangelogEntry; buckets: Buckets }[] = [];
  let current: { entry: ChangelogEntry; buckets: Buckets } | null = null;
  let kind: ChangeKind | null = 'other';

  for (const line of md.split('\n')) {
    const version = line.match(/^##\s+\[?([^\]\s]+)\]?(?:\s*-\s*(\S+))?\s*$/);
    if (version) {
      current = null;
      kind = 'other';
      if (version[1].toLowerCase() !== 'unreleased') {
        const buckets: Buckets = new Map();
        current = {
          entry: { version: version[1], date: version[2] ?? null, groups: [] },
          buckets,
        };
        entries.push(current);
      }
      continue;
    }
    const section = line.match(/^#{3,6}\s+(.+?)\s*$/);
    if (section) {
      kind = kindOf(section[1]);
      continue;
    }
    if (current && kind && /^[-*]\s+/.test(line)) {
      const item = cleanItem(line);
      if (item) addItem(current.buckets, kind, item);
    }
  }

  for (const e of entries) e.entry.groups = toGroups(e.buckets);
  return entries.map((e) => e.entry).filter((e) => e.groups.length > 0);
}

const parsed = parseChangelog(raw);

export function latestEntry(): ChangelogEntry | null {
  return parsed[0] ?? null;
}

export function olderEntries(): ChangelogEntry[] {
  return parsed.slice(1);
}
