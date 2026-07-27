export type ConfirmTone = 'default' | 'warning' | 'danger';

export interface ConfirmCheckbox {
  label: string;
  defaultChecked?: boolean;
}

export interface ConfirmOptions {
  title: string;
  message?: string;
  detail?: string;
  // Raw text shown verbatim in a scrollable monospace block (crash reports, logs).
  code?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  tone?: ConfirmTone;
  // If set, confirm button is locked for this many ms with a fill animation.
  holdMs?: number;
  // Optional checkbox shown above the buttons.
  checkbox?: ConfirmCheckbox;
  // If true, only the confirm button is shown (acts as an alert / OK box).
  hideCancel?: boolean;
}

export interface ConfirmResult {
  confirmed: boolean;
  checked: boolean;
}

function createConfirmState() {
  let open = $state(false);
  let options = $state<ConfirmOptions | null>(null);
  let checkboxChecked = $state(false);
  let resolver: ((value: ConfirmResult) => void) | null = null;
  // Dialogs requested while one is already showing wait here; each gets its
  // turn instead of clobbering the visible one (which would leave the earlier
  // caller's promise unresolved forever).
  const pending: { opts: ConfirmOptions; resolve: (v: ConfirmResult) => void }[] = [];

  function ask(opts: ConfirmOptions): Promise<boolean> {
    return askFull(opts).then((r) => r.confirmed);
  }

  function present(opts: ConfirmOptions, resolve: (v: ConfirmResult) => void) {
    options = opts;
    checkboxChecked = opts.checkbox?.defaultChecked ?? false;
    open = true;
    resolver = resolve;
  }

  function askFull(opts: ConfirmOptions): Promise<ConfirmResult> {
    return new Promise((resolve) => {
      if (open) pending.push({ opts, resolve });
      else present(opts, resolve);
    });
  }

  function close(confirmed: boolean) {
    const checked = checkboxChecked;
    open = false;
    const r = resolver;
    resolver = null;
    queueMicrotask(() => {
      const next = pending.shift();
      if (next) present(next.opts, next.resolve);
      else if (!open) {
        options = null;
        checkboxChecked = false;
      }
    });
    r?.({ confirmed, checked });
  }

  function setChecked(v: boolean) {
    checkboxChecked = v;
  }

  return {
    get open() {
      return open;
    },
    get options() {
      return options;
    },
    get checkboxChecked() {
      return checkboxChecked;
    },
    setChecked,
    ask,
    askFull,
    confirm: () => close(true),
    cancel: () => close(false),
  };
}

export const confirmState = createConfirmState();
export const confirmDialog = (opts: ConfirmOptions) => confirmState.ask(opts);
export const confirmDialogWithCheckbox = (opts: ConfirmOptions & { checkbox: ConfirmCheckbox }) =>
  confirmState.askFull(opts);
