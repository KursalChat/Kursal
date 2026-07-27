// Centralized frontend logger. Single control point for app logging so we can
// gate verbosity by build and avoid scattered console.* calls.
// - Console: debug/info are silenced in production builds; warn/error always emit.
// - .log file: info/warn/error are mirrored into the Rust logger (same file as
//   the backend) via the `log_frontend` command so webview logs sit alongside
//   backend logs. Debug is only mirrored in dev to avoid prod IPC noise. The
//   Rust logger still applies RUST_LOG on top.

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
