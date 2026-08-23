import { browser } from '$app/environment';
import { setTimeFormatPref, type TimeFormat } from '$lib/utils/dateFormat.svelte';
import { readRaw, writeRaw } from '$lib/utils/storage';
import { setSystemBarsLight } from '$lib/utils/system-bars';

export type ThemeMode = 'light' | 'dark' | 'system';
export type ZoomMode = 'smaller' | 'normal' | 'larger';
export type LayoutMode = 'bubble' | 'flat';
export type PaletteId =
  | 'classic'
  | 'ocean'
  | 'forest'
  | 'sunset'
  | 'rose'
  | 'violet'
  | 'midnight'
  | 'slate'
  | 'mocha'
  | 'amber'
  | 'mint';

export interface PalettePreset {
  id: PaletteId;
  label: string;
  previewFrom: string;
  previewTo: string;
  accent: string;
}

// Preview colors approximate the dark-mode look of each palette.
export const PALETTES: PalettePreset[] = [
  {
    id: 'classic',
    label: 'Classic',
    previewFrom: '#111827',
    previewTo: '#0b1120',
    accent: '#818cf8',
  },
  { id: 'ocean', label: 'Ocean', previewFrom: '#0a1a2e', previewTo: '#050a18', accent: '#38bdf8' },
  {
    id: 'forest',
    label: 'Forest',
    previewFrom: '#08170f',
    previewTo: '#030a06',
    accent: '#34d399',
  },
  { id: 'mint', label: 'Mint', previewFrom: '#062925', previewTo: '#021412', accent: '#2dd4bf' },
  {
    id: 'sunset',
    label: 'Sunset',
    previewFrom: '#1c0a08',
    previewTo: '#080302',
    accent: '#fb923c',
  },
  { id: 'amber', label: 'Amber', previewFrom: '#1a1605', previewTo: '#060501', accent: '#facc15' },
  { id: 'mocha', label: 'Mocha', previewFrom: '#1a0f08', previewTo: '#0a0604', accent: '#f59e0b' },
  { id: 'rose', label: 'Rose', previewFrom: '#1a080f', previewTo: '#080305', accent: '#fb7185' },
  {
    id: 'violet',
    label: 'Violet',
    previewFrom: '#110a20',
    previewTo: '#060310',
    accent: '#c084fc',
  },
  {
    id: 'midnight',
    label: 'Midnight',
    previewFrom: '#090d1d',
    previewTo: '#02040c',
    accent: '#a5b4fc',
  },
  { id: 'slate', label: 'Slate', previewFrom: '#0f172a', previewTo: '#020617', accent: '#cbd5e1' },
];

const THEME_KEY = 'kursal_theme';
const ZOOM_KEY = 'kursal_zoom';
const PALETTE_KEY = 'kursal_palette';
const LAYOUT_KEY = 'kursal_layout';
const TIME_FORMAT_KEY = 'kursal_time_format';

const ZOOM_SCALE: Record<ZoomMode, number> = {
  smaller: 0.9,
  normal: 1.0,
  larger: 1.12,
};

function createAppearanceState() {
  let theme = $state<ThemeMode>('system');
  let zoom = $state<ZoomMode>('normal');
  let palette = $state<PaletteId>('classic');
  let layout = $state<LayoutMode>('bubble');
  let timeFormat = $state<TimeFormat>('24h');
  let systemDark = $state(true);
  let initialized = $state(false);
  let mediaQuery: MediaQueryList | null = null;

  function effectiveDark(): boolean {
    if (theme === 'system') return systemDark;
    return theme === 'dark';
  }

  function applyImmediate() {
    const root = document.documentElement;
    const dark = effectiveDark();
    root.dataset.theme = dark ? 'dark' : 'light';
    root.dataset.palette = palette;
    root.style.setProperty('--zoom', String(ZOOM_SCALE[zoom]));
    setSystemBarsLight(!dark);
  }

  function apply() {
    if (!browser) return;
    const root = document.documentElement;
    const themeChanging =
      root.dataset.theme !== (effectiveDark() ? 'dark' : 'light') ||
      root.dataset.palette !== palette;

    type DocWithVT = Document & {
      startViewTransition?: (cb: () => void) => unknown;
    };
    const doc = document as DocWithVT;
    if (themeChanging && initialized && typeof doc.startViewTransition === 'function') {
      doc.startViewTransition(() => applyImmediate());
    } else {
      applyImmediate();
    }
  }

  function init() {
    if (!browser || initialized) return;
    const storedTheme = readRaw(THEME_KEY) as ThemeMode | null;
    const storedZoom = readRaw(ZOOM_KEY) as ZoomMode | null;
    const storedPalette = readRaw(PALETTE_KEY) as PaletteId | null;
    if (storedTheme) theme = storedTheme;
    if (storedZoom) zoom = storedZoom;
    if (storedPalette && PALETTES.some((p) => p.id === storedPalette)) {
      palette = storedPalette;
    }
    const storedLayout = readRaw(LAYOUT_KEY) as LayoutMode | null;
    if (storedLayout === 'bubble' || storedLayout === 'flat') layout = storedLayout;

    const storedTimeFormat = readRaw(TIME_FORMAT_KEY);
    if (storedTimeFormat === '12h') timeFormat = '12h';
    else if (storedTimeFormat === '24h') timeFormat = '24h';
    setTimeFormatPref(timeFormat);

    mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    systemDark = mediaQuery.matches;
    mediaQuery.addEventListener('change', (e) => {
      systemDark = e.matches;
      apply();
    });

    apply();
    initialized = true;
  }

  function setTheme(value: ThemeMode) {
    theme = value;
    writeRaw(THEME_KEY, value);
    apply();
  }

  function setZoom(value: ZoomMode) {
    zoom = value;
    writeRaw(ZOOM_KEY, value);
    apply();
  }

  function setPalette(value: PaletteId) {
    palette = value;
    writeRaw(PALETTE_KEY, value);
    apply();
  }

  function setLayout(value: LayoutMode) {
    layout = value;
    writeRaw(LAYOUT_KEY, value);
  }

  function setTimeFormat(value: TimeFormat) {
    timeFormat = value;
    setTimeFormatPref(value);
    writeRaw(TIME_FORMAT_KEY, value);
  }

  return {
    get theme() {
      return theme;
    },
    get zoom() {
      return zoom;
    },
    get palette() {
      return palette;
    },
    get layout() {
      return layout;
    },
    get timeFormat() {
      return timeFormat;
    },
    get isDark() {
      return effectiveDark();
    },
    init,
    setTheme,
    setZoom,
    setPalette,
    setLayout,
    setTimeFormat,
  };
}

export const appearanceState = createAppearanceState();
