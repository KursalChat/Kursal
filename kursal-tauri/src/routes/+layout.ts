import { redirect } from '@sveltejs/kit';
import type { LayoutLoad } from './$types';

// Tauri doesn't have a Node.js server to do proper SSR
// so we use adapter-static with a fallback to index.html to put the site in SPA mode
// See: https://svelte.dev/docs/kit/single-page-apps
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
export const ssr = false;

// Runs before first paint (SPA mode = client only), so fresh installs land on
// onboarding without flashing the app shell.
export const load: LayoutLoad = ({ url }) => {
  if (
    typeof localStorage !== 'undefined' &&
    localStorage.getItem('kursal_onboarded') !== 'done' &&
    url.pathname !== '/onboarding'
  ) {
    redirect(307, '/onboarding');
  }
};
