import { createRequire } from 'node:module';

const MODULE_ID = 'virtual:emoji-index';
const RESOLVED_ID = '\0' + MODULE_ID;

/** @type {Record<string, number>} */
const SKIN_TONE_BY_MODIFIER = {
  '1F3FB': 1,
  '1F3FC': 2,
  '1F3FD': 3,
  '1F3FE': 4,
  '1F3FF': 5,
};

/**
 * @typedef {object} RawEmoji
 * @property {string} hexcode
 * @property {string} label
 * @property {string} unicode
 * @property {string[]} [tags]
 * @property {number} [group]
 * @property {{ unicode: string, hexcode: string }[]} [skins]
 */

/**
 * Entry layout: [unicode, label, group, tags, shortcodes, skins]
 * where skins is 0 or a list of [tone, unicode].
 *
 * @returns {import('vite').Plugin}
 */
export function emojiIndexPlugin() {
  return {
    name: 'kursal-emoji-index',
    resolveId(id) {
      return id === MODULE_ID ? RESOLVED_ID : null;
    },
    load(id) {
      if (id !== RESOLVED_ID) return null;

      const require = createRequire(import.meta.url);
      /** @type {RawEmoji[]} */
      const emojis = require('emojibase-data/en/compact.json');
      /** @type {Record<string, string | string[]>} */
      const shortcodes = require('emojibase-data/en/shortcodes/emojibase.json');
      /** @type {{ groups: Record<string, string> }} */
      const groupsMeta = require('emojibase-data/meta/groups.json');
      const groups = groupsMeta.groups;

      const packed = [];
      for (const raw of emojis) {
        if (raw.group === undefined) continue;
        const slug = groups[String(raw.group)];
        if (!slug || slug === 'component') continue;

        const found = shortcodes[raw.hexcode];
        const codes = found === undefined ? [] : Array.isArray(found) ? found : [found];

        const skins = [];
        for (const skin of raw.skins ?? []) {
          const parts = skin.hexcode.split('-');
          if (parts.length !== 2) continue;
          const tone = SKIN_TONE_BY_MODIFIER[parts[1]];
          if (tone) skins.push([tone, skin.unicode]);
        }

        packed.push([
          raw.unicode,
          raw.label,
          slug,
          raw.tags ?? [],
          codes,
          skins.length ? skins : 0,
        ]);
      }

      return `export default ${JSON.stringify(packed)};`;
    },
  };
}
