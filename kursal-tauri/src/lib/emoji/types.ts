import { t } from '$lib/i18n';

export type ToneId = 0 | 1 | 2 | 3 | 4 | 5;

export interface Emoji {
  unicode: string;
  label: string;
  group: string;
  tags: string[];
  shortcodes: string[];
  skins?: { tone: ToneId; unicode: string }[];
}

export interface EmojiGroup {
  id: string;
  label: string;
  icon: string;
  emojis: Emoji[];
}

export interface EmojiIndex {
  groups: EmojiGroup[];
  flat: Emoji[];
  byShortcode: Map<string, Emoji>;
}

export interface RawEmoji {
  hexcode: string;
  label: string;
  unicode: string;
  tags?: string[];
  group?: number;
  skins?: { tone?: ToneId | ToneId[]; unicode: string; hexcode: string }[];
}

export const GROUP_META: { slug: string; label: string; icon: string }[] = [
  {
    slug: 'smileys-emotion',
    label: t('emojiPicker.groups.smileys-emotion'),
    icon: '😀',
  },
  {
    slug: 'people-body',
    label: t('emojiPicker.groups.people-body'),
    icon: '👋',
  },
  {
    slug: 'animals-nature',
    label: t('emojiPicker.groups.animals-nature'),
    icon: '🐶',
  },
  { slug: 'food-drink', label: t('emojiPicker.groups.food-drink'), icon: '🍔' },
  { slug: 'activities', label: t('emojiPicker.groups.activities'), icon: '⚽' },
  {
    slug: 'travel-places',
    label: t('emojiPicker.groups.travel-places'),
    icon: '✈️',
  },
  { slug: 'objects', label: t('emojiPicker.groups.objects'), icon: '💡' },
  { slug: 'symbols', label: t('emojiPicker.groups.symbols'), icon: '❤️' },
  { slug: 'flags', label: t('emojiPicker.groups.flags'), icon: '🏳️' },
];
