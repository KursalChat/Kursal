<script lang="ts">
  import { Image, Camera, Paperclip } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import { trapFocus } from '$lib/utils/focusTrap';
  import { OS } from '$lib/api/window';
  import type { PickerMode } from '$lib/utils/file-transfer-paths';

  interface Props {
    onClose: () => void;
    onPickFiles: (files: File[]) => void;
    onPickNative: (mode: PickerMode) => void;
  }

  let { onClose, onPickFiles, onPickNative }: Props = $props();
  const nativePickers = OS === 'ios';

  let galleryInput = $state<HTMLInputElement | null>(null);
  let filesInput = $state<HTMLInputElement | null>(null);
  let cameraInput = $state<HTMLInputElement | null>(null);

  function pickNative(mode: PickerMode) {
    onPickNative(mode);
    onClose();
  }

  function onFilesChange(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const files = Array.from(input.files ?? []);
    input.value = '';
    if (files.length) onPickFiles(files);
    onClose();
  }
</script>

<div
  class="sheet-backdrop"
  onclick={onClose}
  onkeydown={(e) => {
    if (e.key === 'Escape') onClose();
  }}
  role="button"
  tabindex="-1"
  aria-label={t('chat.attach.backdropAriaLabel')}
></div>
<div
  class="attach-sheet"
  role="dialog"
  aria-modal="true"
  aria-label={t('chat.attach.dialogAriaLabel')}
  tabindex="-1"
  use:trapFocus
  onkeydown={(e) => {
    if (e.key === 'Escape') onClose();
  }}
>
  <div class="sheet-handle"></div>
  <div class="sheet-actions">
    <button
      class="sheet-row"
      onclick={() => (nativePickers ? pickNative('media') : galleryInput?.click())}
    >
      <Image size={20} /><span>{t('chat.attach.photos')}</span>
    </button>
    <button class="sheet-row" onclick={() => cameraInput?.click()}>
      <Camera size={20} /><span>{t('chat.attach.camera')}</span>
    </button>
    <button
      class="sheet-row"
      onclick={() => (nativePickers ? pickNative('document') : filesInput?.click())}
    >
      <Paperclip size={20} /><span>{t('chat.attach.files')}</span>
    </button>
  </div>
  <input
    bind:this={galleryInput}
    class="hidden-input"
    type="file"
    accept="image/*,video/*"
    multiple
    onchange={onFilesChange}
  />
  <input
    bind:this={filesInput}
    class="hidden-input"
    type="file"
    multiple
    onchange={onFilesChange}
  />
  <input
    bind:this={cameraInput}
    class="hidden-input"
    type="file"
    accept="image/*"
    capture="environment"
    onchange={onFilesChange}
  />
</div>

<style>
  .sheet-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    z-index: 300;
    animation: fadeIn 0.15s ease;
  }
  .attach-sheet {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    background: var(--bg-secondary);
    border-top: 1px solid var(--border);
    border-radius: var(--radius-md) var(--radius-md) 0 0;
    padding: 8px max(10px, var(--safe-right)) max(16px, var(--safe-bottom))
      max(10px, var(--safe-left));
    z-index: 310;
    animation: sheetUp 0.22s cubic-bezier(0.3, 0, 0.2, 1);
    box-shadow: 0 -10px 40px rgba(0, 0, 0, 0.4);
  }
  .sheet-handle {
    width: 36px;
    height: 4px;
    background: rgba(148, 163, 184, 0.3);
    border-radius: 4px;
    margin: 6px auto 12px;
  }
  .sheet-actions {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .sheet-row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 16px;
    color: var(--text-primary);
    border-radius: var(--radius-md);
    font-size: var(--text-md);
    text-align: left;
    transition: background var(--transition);
  }
  .sheet-row:active {
    background: var(--bg-hover);
  }
  @media (hover: hover) {
    .sheet-row:hover {
      background: var(--bg-hover);
    }
  }
  .hidden-input {
    position: absolute;
    width: 1px;
    height: 1px;
    opacity: 0;
    pointer-events: none;
  }
</style>
