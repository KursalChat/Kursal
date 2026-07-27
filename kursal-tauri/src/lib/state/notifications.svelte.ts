interface ToastAction {
  label: string;
  onClick: () => void;
}

interface Toast {
  id: string;
  message: string;
  kind: 'info' | 'success' | 'error' | 'warning';
  action?: ToastAction;
}

function createNotifications() {
  let toasts = $state<Toast[]>([]);

  function push(
    message: string,
    kind: Toast['kind'] = 'info',
    opts?: { action?: ToastAction; duration?: number }
  ): string {
    const id = crypto.randomUUID();
    toasts.push({ id, message, kind, action: opts?.action });
    setTimeout(() => dismiss(id), opts?.duration ?? 4000);
    return id;
  }

  function dismiss(id: string) {
    toasts = toasts.filter((t) => t.id !== id);
  }

  return {
    get toasts() {
      return toasts;
    },
    push,
    dismiss,
  };
}

export const notifications = createNotifications();
