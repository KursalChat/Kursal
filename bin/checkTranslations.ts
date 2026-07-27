import { LOCALES } from "../kursal-tauri/src/lib/i18n/locales";
import { readFileSync, readdirSync } from "fs";
import path from "path";

const TRANSLATION_ROOT = "./locales";
const ROOT = "./kursal-tauri/src";

const STRICT = process.argv.includes("--strict");
let errorCount = 0;
let untranslatedCount = 0;

function hasPattern(pattern: string): boolean {
  function walkDir(dir: string): boolean {
    const entries = readdirSync(dir, { withFileTypes: true });

    for (const entry of entries) {
      const fullPath = path.join(dir, entry.name);

      if (entry.isDirectory()) {
        if (walkDir(fullPath)) {
          return true;
        }
        continue;
      }

      if ([".ts", ".svelte"].includes(path.extname(entry.name))) {
        const content = readFileSync(fullPath, "utf-8");

        if (content.includes(pattern)) {
          return true;
        }
      }
    }

    return false;
  }

  return walkDir(ROOT);
}

function readTranslation(name: string): Record<string, any> {
  const content = readFileSync(`${TRANSLATION_ROOT}/${name}.json`, "utf-8");
  return JSON.parse(content);
}

function getKeys(obj: Record<string, any>, prefix = ""): string[] {
  const values: string[] = [];

  Object.keys(obj).forEach((key) => {
    const newPrefix = prefix ? `${prefix}.${key}` : key;

    if (typeof obj[key] == "string") {
      values.push(newPrefix);
    } else {
      values.push(...getKeys(obj[key], newPrefix));
    }
  });

  return values;
}

const CHECK_1 = "Checked for duplicate strings in EN";
console.time(CHECK_1);

const TRANSLATION_EN = readTranslation("en");
const STRINGS_EN = getKeys(TRANSLATION_EN);

// test if all strings are used in "en"
for (const string of STRINGS_EN) {
  if (!string.startsWith("common.") && !hasPattern(string)) {
    console.warn(`⚠️  Unused EN string "${string}"`);
    errorCount++;
  }
}

console.timeEnd(CHECK_1);

// see if strings do not match between en - other lang
for (const locale of LOCALES.map((l) => l.id).filter((id) => id != "en")) {
  const CHECK_2 = `Checked for missmatches between EN and ${locale.toUpperCase()}`;
  console.time(CHECK_2);

  const TRANSLATION_LANG = readTranslation(locale);
  const STRINGS_LANG = getKeys(TRANSLATION_LANG);

  for (const val of STRINGS_LANG.filter((str) => !STRINGS_EN.includes(str))) {
    console.warn(`⚠️  String in ${locale.toUpperCase()} but not in EN: ${val}`);
    errorCount++;
  }

  const untranslated_english = STRINGS_EN.filter(
    (str) => !STRINGS_LANG.includes(str),
  ).length;
  untranslatedCount += untranslated_english;
  console.warn(
    `⚠️  ${untranslated_english} strings in EN but not in ${locale.toUpperCase()}`,
  );

  console.timeEnd(CHECK_2);
}

if (untranslatedCount > 0) {
  console.warn(`\n⚠️  ${untranslatedCount} untranslated string(s) across locales.`);
}

if (STRICT && errorCount > 0) {
  console.error(`\n❌ ${errorCount} unused or orphaned translation key(s) found.`);
  process.exit(1);
}
