import { redirect } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';

export const ssr = false;

export const load: LayoutLoad = ({ url }) => {
  if (
    typeof localStorage !== 'undefined' &&
    localStorage.getItem('kursal_onboarded') !== 'done' &&
    url.pathname !== '/onboarding'
  ) {
    redirect(307, '/onboarding');
  }
};
