import { defineConfig } from 'vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { fileURLToPath } from 'node:url';
import { emojiIndexPlugin } from './emoji-index-plugin.js';

// Repo root, so src can import ../../CHANGELOG.md?raw in dev.
const repoRoot = fileURLToPath(new URL('..', import.meta.url));

// // @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

const TERMS_VERSION_URL = 'https://kursal.chat/terms/v';
const TERMS_FALLBACK = '2026-07-27';

/**
 * @param {boolean} isBuild
 * @returns {Promise<string>}
 */
async function resolveTermsVersion(isBuild) {
  try {
    const res = await fetch(TERMS_VERSION_URL, {
      signal: AbortSignal.timeout(10000),
    });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    const version = (await res.text()).trim();
    if (!/^\d{4}-\d{2}-\d{2}$/.test(version)) {
      throw new Error(`expected an ISO date, got ${JSON.stringify(version.slice(0, 20))}`);
    }
    return version;
  } catch (e) {
    const reason = e instanceof Error ? e.message : String(e);
    if (isBuild) {
      throw new Error(`Could not read the terms version from ${TERMS_VERSION_URL}: ${reason}`);
    }
    console.warn(`[terms] ${reason} - falling back to ${TERMS_FALLBACK}`);
    return TERMS_FALLBACK;
  }
}

// https://vite.dev/config/
export default defineConfig(async ({ command }) => ({
  plugins: [emojiIndexPlugin(), sveltekit()],
  define: {
    __TERMS_UPDATED__: JSON.stringify(await resolveTermsVersion(command === 'build')),
  },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**'],
    },
    fs: {
      allow: [repoRoot],
    },
  },
}));
