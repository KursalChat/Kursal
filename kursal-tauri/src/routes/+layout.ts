import { redirect } from '@sveltejs/kit';
import { localeReady } from '#lib/i18n/index.js';
import { readRaw } from '#lib/utils/storage.js';
import { ONBOARDED_KEY } from '#lib/utils/storage-keys.js';
import type { LayoutLoad } from './$types';

export const ssr = false;

export const load: LayoutLoad = async ({ url }) => {
  await localeReady;
  if (readRaw(ONBOARDED_KEY) !== 'done' && url.pathname !== '/onboarding') {
    redirect(307, '/onboarding');
  }
};
