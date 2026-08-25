export type PermissionGroup = 'bluetooth' | 'microphone' | 'camera';

declare global {
  interface Window {
    __kursalPerms?: { request(group: PermissionGroup): string };
    __kursalOnPerms?: (token: string, granted: boolean) => void;
  }
}

const pending = new Map<string, (granted: boolean) => void>();

function bridge() {
  return typeof window === 'undefined' ? undefined : window.__kursalPerms;
}

function install(): void {
  if (typeof window === 'undefined' || window.__kursalOnPerms) return;
  window.__kursalOnPerms = (token, granted) => {
    const resolve = pending.get(token);
    pending.delete(token);
    resolve?.(granted);
  };
}

export function ensurePermission(group: PermissionGroup): Promise<boolean> {
  const b = bridge();
  if (!b) return Promise.resolve(true);
  install();
  return new Promise((resolve) => {
    let token: string;
    try {
      token = b.request(group);
    } catch {
      resolve(false);
      return;
    }
    pending.set(token, resolve);
  });
}
