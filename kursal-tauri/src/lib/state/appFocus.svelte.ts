import { getCurrentWindow } from '@tauri-apps/api/window';

function createAppFocusState() {
  let nativeFocused = $state(true);
  let visible = $state(true);

  function syncVisible() {
    visible = document.visibilityState === 'visible';
  }

  function init(): () => void {
    syncVisible();
    document.addEventListener('visibilitychange', syncVisible);

    let stopped = false;
    let unlistenFocus: (() => void) | null = null;
    const win = getCurrentWindow();
    void win
      .isFocused()
      .then((f) => {
        if (!stopped) nativeFocused = f;
      })
      .catch(() => {});
    void win
      .onFocusChanged(({ payload }) => (nativeFocused = payload))
      .then((fn) => (stopped ? fn() : (unlistenFocus = fn)))
      .catch(() => {});

    return () => {
      stopped = true;
      unlistenFocus?.();
      document.removeEventListener('visibilitychange', syncVisible);
    };
  }

  return {
    get focused() {
      return nativeFocused && visible;
    },
    init,
  };
}

export const appFocusState = createAppFocusState();
