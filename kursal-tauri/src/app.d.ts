declare global {
  namespace App {}

  // Injected by vite.config.js from https://kursal.chat/terms/v at build time.
  const __TERMS_UPDATED__: string;
}

export {};
