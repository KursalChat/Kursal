function createUpdateDownloadState() {
  let active = $state(false);
  let version = $state<string | null>(null);
  let downloaded = $state(0);
  let total = $state<number | null>(null);
  let done = $state(false);
  let installTimer: ReturnType<typeof setTimeout> | undefined;

  function start(v: string | null) {
    clearTimeout(installTimer);
    active = true;
    version = v;
    downloaded = 0;
    total = null;
    done = false;
  }

  function setProgress(bytes: number, contentLength: number | null) {
    active = true;
    done = false;
    downloaded = bytes;
    if (contentLength) total = contentLength;
  }

  function finish() {
    active = true;
    done = true;
    if (total) downloaded = total;
    clearTimeout(installTimer);
    installTimer = setTimeout(reset, 60_000);
  }

  function reset() {
    clearTimeout(installTimer);
    active = false;
    version = null;
    downloaded = 0;
    total = null;
    done = false;
  }

  return {
    get active() {
      return active;
    },
    get version() {
      return version;
    },
    get downloaded() {
      return downloaded;
    },
    get total() {
      return total;
    },
    get done() {
      return done;
    },
    start,
    setProgress,
    finish,
    reset,
  };
}

export const updateDownloadState = createUpdateDownloadState();
