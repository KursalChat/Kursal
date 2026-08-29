<script lang="ts">
  import { Sun, Moon, Monitor, Check, Languages } from 'lucide-svelte';
  import { onMount } from 'svelte';
  import {
    t,
    locale,
    LOCALES,
    translationPercentage,
    loadTranslationPercentages,
    type Locale,
  } from '$lib/i18n';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import {
    appearanceState,
    PALETTES,
    type PaletteId,
    type LayoutMode,
  } from '$lib/state/appearance.svelte';
  import { notifyError } from '$lib/utils/errors';
  import Button from '$lib/components/Button.svelte';
  import SettingCard from './SettingCard.svelte';
  import SettingRow from './SettingRow.svelte';
  import Segmented from './Segmented.svelte';
  import Select from './Select.svelte';

  const LAYOUTS = $derived<{ id: LayoutMode; label: string; desc: string }[]>([
    {
      id: 'bubble',
      label: t('settings.appearance.messageLayoutBubble'),
      desc: t('settings.appearance.messageLayoutBubbleDesc'),
    },
    {
      id: 'flat',
      label: t('settings.appearance.messageLayoutFlat'),
      desc: t('settings.appearance.messageLayoutFlatDesc'),
    },
  ]);

  onMount(loadTranslationPercentages);

  async function openTranslate() {
    try {
      await openUrl(`https://translate.kursal.chat/engage/kursal/${locale.current}/`);
    } catch (e) {
      notifyError(e);
    }
  }
</script>

<div class="sec-head">
  <h2>{t('settings.appearance.heading')}</h2>
  <p>{t('settings.appearance.description')}</p>
</div>

<SettingCard title={t('settings.appearance.displayCard')}>
  <SettingRow
    title={t('settings.appearance.themeRow')}
    description={t('settings.appearance.themeDescription')}
  >
    <Segmented
      value={appearanceState.theme}
      options={[
        { value: 'light', label: t('settings.appearance.themeLight'), icon: Sun },
        { value: 'dark', label: t('settings.appearance.themeDark'), icon: Moon },
        { value: 'system', label: t('settings.appearance.themeSystem'), icon: Monitor },
      ]}
      onchange={(v) => appearanceState.setTheme(v)}
    />
  </SettingRow>
  <SettingRow
    title={t('settings.appearance.textSizeRow')}
    description={t('settings.appearance.textSizeDescription')}
  >
    <Segmented
      value={appearanceState.zoom}
      options={[
        { value: 'smaller', label: t('settings.appearance.textSizeSmall') },
        { value: 'normal', label: t('settings.appearance.textSizeNormal') },
        { value: 'larger', label: t('settings.appearance.textSizeLarge') },
      ]}
      onchange={(v) => appearanceState.setZoom(v)}
    />
  </SettingRow>
  <SettingRow
    title={t('settings.appearance.timeFormatRow')}
    description={t('settings.appearance.timeFormatDescription')}
  >
    <Segmented
      value={appearanceState.timeFormat}
      options={[
        { value: '24h', label: t('settings.appearance.timeFormat24h') },
        { value: '12h', label: t('settings.appearance.timeFormat12h') },
      ]}
      onchange={(v) => appearanceState.setTimeFormat(v)}
    />
  </SettingRow>
</SettingCard>

<SettingCard title={t('settings.appearance.languageCard')}>
  <SettingRow
    title={t('settings.appearance.languageRow')}
    description={t('settings.appearance.languageDescription')}
  >
    <Select
      value={locale.current}
      options={LOCALES.map((l: { id: Locale; label: string }) => {
        const percent = translationPercentage(l.id);
        return { value: l.id, label: percent === undefined ? l.label : `${l.label} (${percent}%)` };
      })}
      onchange={(v) => locale.set(v as Locale)}
      minWidth="180px"
    />
  </SettingRow>
  <SettingRow
    title={t('settings.appearance.translateRow')}
    description={t('settings.appearance.translateDescription')}
  >
    <Button variant="secondary" onclick={openTranslate}>
      <Languages size={13} />
      {t('settings.appearance.translateButton')}
    </Button>
  </SettingRow>
</SettingCard>

<SettingCard
  title={t('settings.appearance.colorThemeCard')}
  description={t('settings.appearance.colorThemeDescription')}
>
  <div class="tiles" role="radiogroup" aria-label={t('settings.appearance.colorThemeAriaLabel')}>
    {#each PALETTES as p}
      {@const active = appearanceState.palette === p.id}
      <button
        type="button"
        role="radio"
        aria-checked={active}
        aria-label={p.label}
        class="tile"
        data-active={active}
        style="--from:{p.previewFrom}; --to:{p.previewTo}; --dot:{p.accent}"
        onclick={() => appearanceState.setPalette(p.id as PaletteId)}
      >
        <span class="tile-preview">
          <span class="tile-dot"></span>
          {#if active}
            <span class="tile-check"><Check size={12} strokeWidth={3} /></span>
          {/if}
        </span>
        <span class="tile-label">{p.label}</span>
      </button>
    {/each}
  </div>
</SettingCard>

<SettingCard
  title={t('settings.appearance.messageLayoutCard')}
  description={t('settings.appearance.messageLayoutDescription')}
>
  <div
    class="layout-tiles"
    role="radiogroup"
    aria-label={t('settings.appearance.messageLayoutAriaLabel')}
  >
    {#each LAYOUTS as lay}
      {@const active = appearanceState.layout === lay.id}
      <button
        type="button"
        role="radio"
        aria-checked={active}
        aria-label={lay.label}
        class="layout-tile"
        data-active={active}
        onclick={() => appearanceState.setLayout(lay.id)}
      >
        <div class="layout-preview">
          {#if lay.id === 'bubble'}
            <div class="prev-row prev-recv">
              <div class="prev-avatar"></div>
              <div class="prev-bubble prev-bubble-recv"></div>
            </div>
            <div class="prev-row prev-sent-row">
              <div class="prev-bubble prev-bubble-sent"></div>
            </div>
            <div class="prev-row prev-recv">
              <div class="prev-avatar"></div>
              <div class="prev-bubble prev-bubble-recv short"></div>
            </div>
          {:else}
            <div class="prev-flat-group">
              <div class="prev-flat-avatar"></div>
              <div class="prev-flat-body">
                <div class="prev-flat-name"></div>
                <div class="prev-flat-line"></div>
                <div class="prev-flat-line short"></div>
              </div>
            </div>
            <div class="prev-flat-group prev-flat-you">
              <div class="prev-flat-avatar prev-flat-avatar-you"></div>
              <div class="prev-flat-body">
                <div class="prev-flat-name"></div>
                <div class="prev-flat-line"></div>
              </div>
            </div>
          {/if}
        </div>
        <span class="layout-label">{lay.label}</span>
        <span class="layout-desc">{lay.desc}</span>
      </button>
    {/each}
  </div>
</SettingCard>

<style>
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
    gap: 10px;
    padding: 14px;
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
    padding: 0;
    border-radius: var(--radius-md);
    background: transparent;
    transition: transform var(--transition);
  }
  @media (hover: hover) {
    .tile:hover {
      transform: translateY(-1px);
    }
  }
  .tile-preview {
    position: relative;
    height: 56px;
    border-radius: var(--radius-md);
    background: linear-gradient(180deg, var(--from) 0%, var(--to) 100%);
    border: 2px solid transparent;
    box-shadow: inset 0 0 0 1px rgba(0, 0, 0, 0.25);
    transition:
      border-color var(--transition),
      box-shadow var(--transition);
  }
  /* Accent wash only where color-mix() exists: a color-mix() value containing
     var() computes to `transparent` on engines without it (Chrome < 111), which
     would leave every palette swatch blank. */
  @supports (background: color-mix(in srgb, red 50%, transparent)) {
    .tile-preview {
      background:
        radial-gradient(
          120% 100% at 10% -10%,
          color-mix(in srgb, var(--dot) 22%, transparent),
          transparent 60%
        ),
        linear-gradient(180deg, var(--from) 0%, var(--to) 100%);
    }
  }
  .tile[data-active='true'] .tile-preview {
    border-color: var(--accent);
    box-shadow:
      inset 0 0 0 1px rgba(0, 0, 0, 0.25),
      0 0 0 3px var(--accent-dim);
  }
  .tile-dot {
    position: absolute;
    left: 10px;
    bottom: 10px;
    width: 14px;
    height: 14px;
    border-radius: var(--radius-md);
    background: var(--dot);
    box-shadow:
      0 0 0 1px rgba(0, 0, 0, 0.3),
      0 0 10px color-mix(in srgb, var(--dot) 60%, transparent);
  }
  .tile-check {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 18px;
    height: 18px;
    border-radius: var(--radius-md);
    background: var(--accent);
    color: #fff;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .tile-label {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-secondary);
    text-align: center;
  }
  .tile[data-active='true'] .tile-label {
    color: var(--text-primary);
  }
  .tile:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: var(--radius-md);
  }

  .layout-tiles {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    padding: 14px;
  }
  .layout-tile {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 8px;
    padding: 0;
    border-radius: var(--radius-md);
    background: transparent;
    transition: transform var(--transition);
  }
  @media (hover: hover) {
    .layout-tile:hover {
      transform: translateY(-1px);
    }
  }
  .layout-preview {
    height: 90px;
    border-radius: var(--radius-md);
    background: var(--bg-primary);
    border: 2px solid var(--border);
    padding: 10px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 5px;
    overflow: hidden;
    transition:
      border-color var(--transition),
      box-shadow var(--transition);
  }
  .layout-tile[data-active='true'] .layout-preview {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-dim);
  }
  .layout-label {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text-secondary);
    text-align: center;
  }
  .layout-tile[data-active='true'] .layout-label {
    color: var(--text-primary);
  }
  .layout-desc {
    font-size: var(--text-2xs);
    color: var(--text-muted);
    text-align: center;
    margin-top: -4px;
  }
  .layout-tile:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    border-radius: var(--radius-md);
  }

  /* Bubble preview elements */
  .prev-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .prev-recv {
    justify-content: flex-start;
  }
  .prev-sent-row {
    justify-content: flex-end;
  }
  .prev-avatar {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--text-muted);
    flex-shrink: 0;
  }
  .prev-bubble {
    height: 10px;
    border-radius: 6px;
    width: 52%;
  }
  .prev-bubble.short {
    width: 32%;
  }
  .prev-bubble-recv {
    background: var(--bg-secondary);
  }
  .prev-bubble-sent {
    background: var(--accent);
  }

  .prev-flat-group {
    display: flex;
    align-items: flex-start;
    gap: 5px;
  }
  .prev-flat-you {
    margin-top: 4px;
  }
  .prev-flat-avatar {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--text-muted);
    flex-shrink: 0;
    margin-top: 2px;
  }
  .prev-flat-avatar-you {
    background: var(--accent);
  }
  .prev-flat-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .prev-flat-name {
    height: 5px;
    width: 30%;
    border-radius: 3px;
    background: var(--text-muted);
  }
  .prev-flat-line {
    height: 7px;
    border-radius: 3px;
    background: var(--bg-secondary);
    width: 85%;
  }
  .prev-flat-line.short {
    width: 55%;
  }
</style>
