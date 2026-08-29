import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { notifyError } from '$lib/utils/errors';

interface CopyTarget {
  trigger: () => void;
}

interface CopyOptions {
  flash?: CopyTarget;
  errorKey?: string;
}

export async function copyText(text: string, options: CopyOptions = {}): Promise<boolean> {
  try {
    await writeText(text);
    options.flash?.trigger();
    return true;
  } catch (e) {
    notifyError(e, options.errorKey);
    return false;
  }
}
