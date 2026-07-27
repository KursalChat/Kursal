// Stub for SvelteKit's $app/environment in unit tests.
// browser=false makes locale detection deterministic (defaults to "en")
// without touching navigator/localStorage.
export const browser = false;
export const building = false;
export const dev = false;
export const version = 'test';
