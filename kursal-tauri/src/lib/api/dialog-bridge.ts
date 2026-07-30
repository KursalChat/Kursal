import { invoke } from '@tauri-apps/api/core';
import { confirmDialog, type ConfirmOptions, type ConfirmTone } from '$lib/state/confirm.svelte';
import { groupLabel, parseReleaseNotes } from '$lib/changelog';
import { t } from '$lib/i18n';

export interface BackendDialogPayload {
  id: number;
  kind: string;
  message?: string;
  params: Record<string, string | number | null>;
  tone: ConfirmTone;
  dismissible: boolean;
}

export const dialogRespond = (id: number, confirmed: boolean): Promise<void> =>
  invoke('dialog_respond', { id, confirmed });

export const runStartupDialogs = (): Promise<void> => invoke('run_startup_dialogs');

function buildOptions(p: BackendDialogPayload): ConfirmOptions {
  const base = { tone: p.tone, hideCancel: !p.dismissible } as const;

  switch (p.kind) {
    case 'crash_report':
      return {
        ...base,
        title: t('backendDialog.crashReportTitle'),
        message: t('backendDialog.crashReportMessage'),
        detail: t('backendDialog.crashReportDetail'),
        code: p.params.report ? String(p.params.report) : undefined,
        confirmLabel: t('backendDialog.crashReportConfirm'),
        cancelLabel: t('backendDialog.crashReportCancel'),
      };
    case 'update_available': {
      const notes = p.params.notes ? String(p.params.notes) : '';
      const groups = notes ? parseReleaseNotes(notes) : [];
      return {
        ...base,
        title: t('backendDialog.updateAvailableTitle'),
        message: t('backendDialog.updateAvailableMessage', {
          version: String(p.params.version ?? ''),
          currentVersion: String(p.params.currentVersion ?? ''),
        }),
        detail: groups.length ? t('backendDialog.updateAvailableNotes') : undefined,
        sections: groups.map((g) => ({ title: groupLabel(g.kind), items: g.items })),
        code: notes && !groups.length ? notes : undefined,
        confirmLabel: t('backendDialog.updateAvailableConfirm'),
        cancelLabel: t('backendDialog.updateAvailableCancel'),
      };
    }
    case 'update_installed':
      return {
        ...base,
        title: t('backendDialog.updateInstalledTitle'),
        message: t('backendDialog.updateInstalledMessage'),
        confirmLabel: t('backendDialog.updateInstalledConfirm'),
        cancelLabel: t('backendDialog.updateInstalledCancel'),
      };
    case 'no_updates':
      return {
        ...base,
        title: t('backendDialog.noUpdatesTitle'),
        message: t('backendDialog.noUpdatesMessage'),
        confirmLabel: t('backendDialog.noUpdatesConfirm'),
      };
    case 'file_open_confirm':
      return {
        ...base,
        title: t('backendDialog.fileOpenTitle', { fileName: String(p.params.fileName ?? '') }),
        message: p.message,
        confirmLabel: t('backendDialog.fileOpenConfirm'),
        cancelLabel: t('backendDialog.fileOpenCancel'),
      };
    case 'file_open_blocked':
      return {
        ...base,
        title: t('backendDialog.fileOpenTitle', { fileName: String(p.params.fileName ?? '') }),
        message: p.message,
        confirmLabel: t('backendDialog.fileBlockedConfirm'),
      };
    default:
      return {
        ...base,
        title: p.kind,
        message: p.message,
        confirmLabel: t('common.confirm'),
      };
  }
}

export async function handleBackendDialog(p: BackendDialogPayload): Promise<void> {
  const confirmed = await confirmDialog(buildOptions(p));
  await dialogRespond(p.id, confirmed);
}
