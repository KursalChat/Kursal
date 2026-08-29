//! NOTE: Add languages here
const LOCALE_IDS = ['en', 'fr', 'nl'] as const;

export type Locale = (typeof LOCALE_IDS)[number];

function nativeName(id: Locale): string {
  const name = new Intl.DisplayNames([id], { type: 'language' }).of(id) ?? id;
  return name.charAt(0).toUpperCase() + name.slice(1);
}

export const LOCALES: { id: Locale; label: string }[] = LOCALE_IDS.map((id) => ({
  id,
  label: nativeName(id),
}));
