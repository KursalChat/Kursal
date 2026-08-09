import { redirect } from '@sveltejs/kit';
import { localeReady } from '$lib/i18n';
import type { LayoutLoad } from './$types';

export const ssr = false;

export const load: LayoutLoad = async ({ url }) => {
  await localeReady;
  if (
    typeof localStorage !== 'undefined' &&
    localStorage.getItem('kursal_onboarded') !== 'done' &&
    url.pathname !== '/onboarding'
  ) {
    redirect(307, '/onboarding');
  }
};
