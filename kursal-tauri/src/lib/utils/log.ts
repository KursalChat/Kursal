// Centralized frontend logger. Console debug/info are silenced in production;
// warn/error always emit. Everything (except dev-only debug) also mirrors into
// the Rust logger via `log_frontend`, landing in the same log file as the backend.

import { invoke } from '@tauri-apps/api/core';

const isDev = import.meta.env.DEV;

function fmt(args: unknown[]): string {
  return args
    .map((a) => {
      if (typeof a === 'string') return a;
      try {
        return JSON.stringify(a);
      } catch {
        return String(a);
      }
    })
    .join(' ');
}

function toRust(level: 'debug' | 'info' | 'warn' | 'error', args: unknown[]) {
  // Fire-and-forget; never let logging throw or block callers.
  void invoke('log_frontend', { level, message: fmt(args) }).catch(() => {});
}

export const log = {
  debug: (...args: unknown[]) => {
    if (isDev) {
      console.debug(...args);
      toRust('debug', args);
    }
  },
  info: (...args: unknown[]) => {
    if (isDev) console.info(...args);
    toRust('info', args);
  },
  warn: (...args: unknown[]) => {
    console.warn(...args);
    toRust('warn', args);
  },
  error: (...args: unknown[]) => {
    console.error(...args);
    toRust('error', args);
  },
};
