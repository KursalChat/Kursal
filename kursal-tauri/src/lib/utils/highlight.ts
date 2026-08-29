import hljs from 'highlight.js/lib/core';
import bash from 'highlight.js/lib/languages/bash';
import c from 'highlight.js/lib/languages/c';
import cpp from 'highlight.js/lib/languages/cpp';
import css from 'highlight.js/lib/languages/css';
import diff from 'highlight.js/lib/languages/diff';
import dockerfile from 'highlight.js/lib/languages/dockerfile';
import go from 'highlight.js/lib/languages/go';
import ini from 'highlight.js/lib/languages/ini';
import java from 'highlight.js/lib/languages/java';
import javascript from 'highlight.js/lib/languages/javascript';
import json from 'highlight.js/lib/languages/json';
import markdown from 'highlight.js/lib/languages/markdown';
import python from 'highlight.js/lib/languages/python';
import rust from 'highlight.js/lib/languages/rust';
import sql from 'highlight.js/lib/languages/sql';
import typescript from 'highlight.js/lib/languages/typescript';
import xml from 'highlight.js/lib/languages/xml';
import yaml from 'highlight.js/lib/languages/yaml';

// Each grammar carries its own aliases, so js/ts/py/rs/sh/yml/html/toml resolve too.
const LANGUAGES = {
  bash,
  c,
  cpp,
  css,
  diff,
  dockerfile,
  go,
  ini,
  java,
  javascript,
  json,
  markdown,
  python,
  rust,
  sql,
  typescript,
  xml,
  yaml,
};

for (const [name, definition] of Object.entries(LANGUAGES)) {
  hljs.registerLanguage(name, definition);
}

// Highlighting a huge block costs
const MAX_LENGTH = 5_000;

const CODE_HINT = /[{}();=<>[\]]/;

function esc(text: string): string {
  return text.replace(/[&<>]/g, (c) => (c === '&' ? '&amp;' : c === '<' ? '&lt;' : '&gt;'));
}

/** Escaped HTML with highlight.js `hljs-*` spans, or plain escaped text if unknown. */
export function highlightCode(source: string, lang: string): string {
  if (source.length > MAX_LENGTH) return esc(source);

  if (lang) {
    if (!hljs.getLanguage(lang)) return esc(source);
    return hljs.highlight(source, { language: lang, ignoreIllegals: true }).value;
  }

  if (!CODE_HINT.test(source)) return esc(source);
  return hljs.highlightAuto(source).value;
}

const FENCE_MARK = /^ {0,3}`{3,}/;

// Runs on every keystroke, so it never auto-detects: only a declared language colours.
/** A draft's fenced block, highlighted, with its ``` lines kept and dimmed. */
export function highlightFence(block: string): string {
  const nl = block.indexOf('\n');
  if (nl < 0) return mark(block);

  const open = block.slice(0, nl);
  const rest = block.slice(nl + 1);
  const lang = open.replace(FENCE_MARK, '').trim().toLowerCase();

  const lastNl = rest.lastIndexOf('\n');
  const tail = lastNl < 0 ? rest : rest.slice(lastNl + 1);
  if (!FENCE_MARK.test(tail)) return `${mark(open)}\n${body(rest, lang)}`;

  if (lastNl < 0) return `${mark(open)}\n${mark(tail)}`;
  return `${mark(open)}\n${body(rest.slice(0, lastNl), lang)}\n${mark(tail)}`;
}

function mark(line: string): string {
  return `<span class="fence-mark">${esc(line)}</span>`;
}

function body(source: string, lang: string): string {
  return lang ? highlightCode(source, lang) : esc(source);
}
