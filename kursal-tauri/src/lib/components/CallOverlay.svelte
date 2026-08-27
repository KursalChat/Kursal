<script lang="ts">
  import {
    Mic,
    MicOff,
    Headphones,
    HeadphoneOff,
    PhoneOff,
    Minus,
    ShieldCheck,
    ShieldAlert,
    Info,
    Video,
    VideoOff,
    FlipHorizontal2,
    ChevronUp,
    Check,
  } from 'lucide-svelte';
  import { t } from '$lib/i18n';
  import Avatar from '$lib/components/Avatar.svelte';
  import { callState } from '$lib/state/call.svelte';
  import { contactsState } from '$lib/state/contacts.svelte';
  import { prefsState } from '$lib/state/prefs.svelte';
  import { profileState } from '$lib/state/profile.svelte';
  import { qualityKey, khz } from '$lib/utils/callQuality';
  import { createCallElapsed } from '$lib/utils/callElapsed.svelte';
  import { drawFrame } from '$lib/call/video';
  import type { CameraInfo } from '$lib/types';

  // Android has no localised camera name, so it sends the facing and no label.
  function cameraName(cam: CameraInfo): string {
    if (cam.label) return cam.label;
    if (cam.facing === 'user') return t('chat.call.cameraFront');
    if (cam.facing === 'environment') return t('chat.call.cameraBack');
    return t('chat.call.cameraNumbered', { id: cam.id });
  }

  const contact = $derived(callState.contactId ? contactsState.getById(callState.contactId) : null);

  const phase = $derived(callState.status);
  const isOutgoing = $derived(phase === 'ringing_out');
  const isConnected = $derived(phase === 'connected');
  const ringing = $derived(phase !== 'connected');

  const open = $derived(
    callState.expanded &&
      (phase === 'ringing_out' || phase === 'connecting' || phase === 'connected')
  );

  const timer = createCallElapsed(
    () => isConnected,
    () => callState.startedAt
  );

  const phaseLabel = $derived(
    isConnected ? timer.label : isOutgoing ? t('chat.call.ringing') : t('chat.call.connecting')
  );

  const ringLevel = $derived(isConnected ? callState.remoteLevel : 0);
  const micGlow = $derived(isConnected && !callState.muted ? callState.micLevel : 0);

  const QUALITY_LABEL_KEYS = {
    low: 'chat.call.qualityLow',
    medium: 'chat.call.qualityMedium',
    high: 'chat.call.qualityHigh',
  } as const;

  const quality = $derived.by(() => {
    const r = callState.sampleRate;
    if (r === null) return null;
    return { label: t(QUALITY_LABEL_KEYS[qualityKey(r)]), khz: khz(r) };
  });

  const videoMode = $derived(callState.localVideo || callState.remoteVideo);

  let remoteCanvas = $state<HTMLCanvasElement | null>(null);
  let selfCanvas = $state<HTMLCanvasElement | null>(null);

  const DEFAULT_AR = 16 / 9;
  let remoteAr = $state(DEFAULT_AR);
  let localAr = $state(DEFAULT_AR);

  function clampAr(w: number, h: number): number {
    if (!w || !h) return DEFAULT_AR;
    return Math.min(16 / 9, Math.max(9 / 16, w / h));
  }

  $effect(() => {
    if (!open || !callState.remoteVideo || !remoteCanvas) {
      callState.setRemoteFrameSink(null);
      remoteAr = DEFAULT_AR;
      return;
    }
    const canvas = remoteCanvas;
    const ctx = canvas.getContext('2d');
    callState.setRemoteFrameSink((frame, rotation) => {
      const size = drawFrame(canvas, ctx, frame, rotation);
      remoteAr = clampAr(size.width, size.height);
      frame.close();
    });
    return () => callState.setRemoteFrameSink(null);
  });

  $effect(() => {
    if (!open || !callState.localVideo || !selfCanvas) {
      callState.setLocalFrameSink(null);
      localAr = DEFAULT_AR;
      return;
    }
    const canvas = selfCanvas;
    const ctx = canvas.getContext('2d');
    callState.setLocalFrameSink((frame, rotation) => {
      const size = drawFrame(canvas, ctx, frame, rotation);
      localAr = clampAr(size.width, size.height);
      frame.close();
    });
    return () => callState.setLocalFrameSink(null);
  });

  const remoteTileAr = $derived(
    callState.remoteVideo ? remoteAr : callState.localVideo ? localAr : DEFAULT_AR
  );
  const localTileAr = $derived(
    callState.localVideo ? localAr : callState.remoteVideo ? remoteAr : DEFAULT_AR
  );

  const sideBySide = $derived(remoteTileAr < 1 || localTileAr < 1);
  const mirrorSelf = $derived(
    prefsState.mirrorSelfView && callState.cameraFacing !== 'environment'
  );

  let cameraMenuOpen = $state(false);
  let cameraMenuEl = $state<HTMLDivElement | null>(null);
  let cameraCaretEl = $state<HTMLButtonElement | null>(null);

  $effect(() => {
    if (!cameraMenuOpen) return;
    function onPointer(e: PointerEvent) {
      const target = e.target as Node;
      if (cameraMenuEl?.contains(target) || cameraCaretEl?.contains(target)) return;
      cameraMenuOpen = false;
    }
    function onKey(e: KeyboardEvent) {
      if (e.key === 'Escape') cameraMenuOpen = false;
    }
    window.addEventListener('pointerdown', onPointer, true);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('pointerdown', onPointer, true);
      window.removeEventListener('keydown', onKey);
    };
  });

  $effect(() => {
    if (!isConnected) cameraMenuOpen = false;
  });

  function openCameraMenu() {
    if (!cameraMenuOpen) void callState.refreshCameras();
    cameraMenuOpen = !cameraMenuOpen;
  }

  function pickCamera(id: string) {
    cameraMenuOpen = false;
    void callState.selectCamera(id);
  }

  const cameraLabel = $derived(
    callState.cameraDenied
      ? t('chat.call.cameraDenied')
      : callState.localVideo
        ? t('chat.call.cameraOff')
        : t('chat.call.cameraOn')
  );
</script>

{#if open && contact}
  <div class="call-overlay" role="dialog" aria-modal="true" aria-label={contact.displayName}>
    <div class="backdrop" data-tauri-drag-region></div>

    <div class="stage" class:video={videoMode}>
      <button
        class="minimize"
        onclick={() => callState.minimize()}
        aria-label={t('chat.call.minimize')}
        title={t('chat.call.minimize')}
      >
        <Minus size={18} />
      </button>

      {#if quality}
        <div class="info-wrap">
          <button class="info-btn" aria-label={t('chat.call.qualityInfo')}>
            <Info size={18} />
          </button>
          <div class="quality-tip" role="tooltip">
            <div class="q-row">
              <span class="q-label">{t('chat.call.quality')}</span>
              <span class="q-value">{quality.label} · {quality.khz} kHz</span>
            </div>
            {#if callState.qualityOverridden}
              <div class="q-override">
                {t('chat.call.qualityOverride', { khz: quality.khz })}
              </div>
            {/if}
          </div>
        </div>
      {/if}

      {#if videoMode}
        <div class="tiles" class:side-by-side={sideBySide}>
          <div class="tile" style="--tile-ar:{remoteTileAr}">
            {#if callState.remoteVideo}
              <canvas class="tile-media" bind:this={remoteCanvas}></canvas>
            {:else}
              <div class="tile-avatar">
                <Avatar name={contact.displayName} src={contact.avatarPath} size={96} />
              </div>
            {/if}
            <span class="tile-name">{contact.displayName}</span>
          </div>
          <div class="tile" style="--tile-ar:{localTileAr}">
            {#if callState.localVideo}
              <canvas class="tile-media" class:mirrored={mirrorSelf} bind:this={selfCanvas}
              ></canvas>
              {#if callState.cameraFacing !== 'environment'}
                <button
                  class="tile-btn"
                  aria-label={t('chat.call.mirrorSelfView')}
                  title={t('chat.call.mirrorSelfView')}
                  aria-pressed={prefsState.mirrorSelfView}
                  onclick={() => prefsState.toggleMirrorSelfView()}
                >
                  <FlipHorizontal2 size={14} />
                </button>
              {/if}
            {:else}
              <div class="tile-avatar">
                <Avatar name={profileState.displayName} src={profileState.avatarPath} size={96} />
              </div>
            {/if}
            <span class="tile-name">{profileState.displayName} ({t('chat.call.you')})</span>
          </div>
        </div>
      {:else}
        <div class="avatar-wrap" class:ringing style="--lvl:{ringLevel}">
          <span class="ring r1"></span>
          <span class="ring r2"></span>
          <span class="ring r3"></span>
          <div class="avatar-core">
            <Avatar name={contact.displayName} src={contact.avatarPath} size={132} />
            {#if isConnected && (callState.peerDeafened || callState.peerMuted)}
              <div
                class="voice-badge"
                title={callState.peerDeafened
                  ? t('chat.call.peerDeafened')
                  : t('chat.call.peerMuted')}
              >
                {#if callState.peerDeafened}
                  <HeadphoneOff size={26} />
                {:else}
                  <MicOff size={26} />
                {/if}
              </div>
            {/if}
          </div>
        </div>
      {/if}

      <div class="identity" data-tauri-drag-region>
        <h2 class="name">{contact.displayName}</h2>
        <div class="phase" class:live={isConnected}>{phaseLabel}</div>

        <div class="trust" class:ok={contact.verified}>
          {#if contact.verified}
            <ShieldCheck size={13} />
            <span>{t('chat.call.verified')}</span>
          {:else}
            <ShieldAlert size={13} />
            <span>{t('chat.call.unverified')}</span>
          {/if}
        </div>
      </div>

      <div class="controls">
        <div class="ctl-group">
          <button
            class="round"
            class:active={callState.muted}
            style="--glow:{micGlow}"
            aria-pressed={callState.muted}
            onclick={() => callState.toggleMute()}
          >
            {#if callState.muted}<MicOff size={20} />{:else}<Mic size={20} />{/if}
          </button>
          <span class="ctl-label">
            {callState.muted ? t('chat.call.unmute') : t('chat.call.mute')}
          </span>
        </div>

        <div class="ctl-group">
          <button
            class="round"
            class:active={callState.deafened}
            aria-pressed={callState.deafened}
            onclick={() => callState.toggleDeafen()}
          >
            {#if callState.deafened}<HeadphoneOff size={20} />{:else}<Headphones size={20} />{/if}
          </button>
          <span class="ctl-label">
            {callState.deafened ? t('chat.call.undeafen') : t('chat.call.deafen')}
          </span>
        </div>

        {#if isConnected}
          <div class="ctl-group">
            {#if callState.canSendVideo}
              <div class="ctl-split">
                <button
                  class="round"
                  class:active={callState.localVideo}
                  class:denied={callState.cameraDenied}
                  aria-pressed={callState.localVideo}
                  onclick={() => callState.toggleCamera()}
                >
                  {#if callState.localVideo}<Video size={20} />{:else}<VideoOff size={20} />{/if}
                </button>
                {#if callState.cameras.length > 1}
                  <button
                    class="caret"
                    class:open={cameraMenuOpen}
                    bind:this={cameraCaretEl}
                    aria-haspopup="menu"
                    aria-expanded={cameraMenuOpen}
                    aria-label={t('chat.call.selectCamera')}
                    title={t('chat.call.selectCamera')}
                    onclick={openCameraMenu}
                  >
                    <ChevronUp size={12} />
                  </button>
                {/if}
                {#if cameraMenuOpen}
                  <div class="cam-menu" role="menu" bind:this={cameraMenuEl}>
                    {#each callState.cameras as cam (cam.id)}
                      <button
                        class="menu-item"
                        role="menuitemradio"
                        aria-checked={cam.id === callState.selectedCameraId}
                        onclick={() => pickCamera(cam.id)}
                      >
                        <span class="menu-check">
                          {#if cam.id === callState.selectedCameraId}<Check size={13} />{/if}
                        </span>
                        <span class="menu-text">{cameraName(cam)}</span>
                      </button>
                    {/each}
                  </div>
                {/if}
              </div>
              <span class="ctl-label">{cameraLabel}</span>
            {:else}
              <button class="round" disabled title={t('chat.call.videoUnsupported')}>
                <VideoOff size={20} />
              </button>
              <span class="ctl-label">{t('chat.call.cameraOn')}</span>
            {/if}
          </div>
        {/if}

        <div class="ctl-group">
          <button class="round decline" onclick={() => callState.end()}>
            <PhoneOff size={20} />
          </button>
          <span class="ctl-label">{t('chat.call.hangup')}</span>
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .call-overlay {
    position: fixed;
    inset: 0;
    z-index: 600;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: calc(24px + var(--safe-top)) max(20px, var(--safe-right))
      calc(24px + var(--safe-bottom)) max(20px, var(--safe-left));
    animation: overlay-in 0.22s ease-out;
  }
  @keyframes overlay-in {
    from {
      opacity: 0;
    }
  }
  .backdrop {
    position: absolute;
    inset: 0;
    background: var(--bg-primary);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
  }
  @supports (background: color-mix(in srgb, red 50%, transparent)) {
    .backdrop {
      background:
        radial-gradient(
          120% 80% at 50% 0%,
          color-mix(in srgb, var(--accent) 22%, transparent),
          transparent 60%
        ),
        var(--bg-primary);
    }
  }
  .stage {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 22px;
    width: 100%;
    max-width: 340px;
  }
  .stage.video {
    max-width: 900px;
    padding-top: 46px;
  }
  .stage.video .minimize,
  .stage.video .info-wrap {
    top: 0;
  }
  .stage.video .tiles {
    margin-top: 0;
  }
  .minimize {
    position: absolute;
    top: -8px;
    right: -4px;
    width: 38px;
    height: 38px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    background: var(--bg-hover);
    transition: all var(--transition);
  }
  @media (hover: hover) {
    .minimize:hover {
      color: var(--text-primary);
      background: var(--surface-soft);
    }
  }
  .minimize:active {
    transform: scale(0.94);
  }

  .info-wrap {
    position: absolute;
    top: -8px;
    left: -4px;
    z-index: 3;
  }
  .info-btn {
    width: 38px;
    height: 38px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    background: var(--bg-hover);
    transition: all var(--transition);
  }
  @media (hover: hover) {
    .info-btn:hover {
      color: var(--text-primary);
      background: var(--surface-soft);
    }
  }
  .quality-tip {
    position: absolute;
    top: calc(100% + 6px);
    left: 0;
    width: max-content;
    max-width: 240px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px 12px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-3);
    opacity: 0;
    transform: translateY(-4px);
    pointer-events: none;
    transition:
      opacity var(--transition),
      transform var(--transition);
  }
  .info-wrap:focus-within .quality-tip {
    opacity: 1;
    transform: translateY(0);
  }
  @media (hover: hover) {
    .info-wrap:hover .quality-tip {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .q-row {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .q-label {
    font-size: var(--text-2xs);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
  }
  .q-value {
    color: var(--text-primary);
    font-weight: 600;
    font-size: var(--text-sm);
  }
  .q-override {
    font-size: var(--text-2xs);
    color: var(--warning);
    line-height: 1.5;
  }

  .avatar-wrap {
    position: relative;
    width: 132px;
    height: 132px;
    display: flex;
    align-items: center;
    justify-content: center;
    margin-top: 12px;
  }
  .avatar-core {
    position: relative;
    z-index: 2;
    border-radius: 50%;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.4);
  }
  .ring {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, var(--accent) 60%, transparent);
    transform: scale(calc(1 + var(--lvl, 0) * 0.55));
    opacity: calc(0.12 + var(--lvl, 0) * 0.8);
    transition:
      transform 0.12s ease-out,
      opacity 0.12s ease-out;
  }
  .ring.r2 {
    transform: scale(calc(1 + var(--lvl, 0) * 0.85));
    opacity: calc(0.06 + var(--lvl, 0) * 0.45);
  }
  .ring.r3 {
    transform: scale(calc(1 + var(--lvl, 0) * 1.15));
    opacity: calc(0.03 + var(--lvl, 0) * 0.25);
  }
  .avatar-wrap.ringing .ring {
    animation: ring-pulse 1.8s ease-out infinite;
  }
  .avatar-wrap.ringing .ring.r2 {
    animation-delay: 0.3s;
  }
  .avatar-wrap.ringing .ring.r3 {
    animation-delay: 0.6s;
  }
  @keyframes ring-pulse {
    0% {
      transform: scale(1);
      opacity: 0.5;
    }
    100% {
      transform: scale(1.7);
      opacity: 0;
    }
  }

  .tiles {
    display: grid;
    grid-template-columns: 1fr 1fr;
    align-items: start;
    gap: 12px;
    width: 100%;
    margin-top: 12px;
    --tile-rows: 1;
    --call-chrome: calc(336px + var(--safe-top) + var(--safe-bottom));
    --tile-cap: calc(
      (calc(var(--app-height, 100dvh) / var(--zoom, 1)) - var(--call-chrome)) / var(--tile-rows)
    );
  }
  .tile {
    position: relative;
    width: 100%;
    aspect-ratio: var(--tile-ar, 16 / 9);
    max-width: calc(max(120px, min(60vh, var(--tile-cap))) * var(--tile-ar, 1.7778));
    margin-inline: auto;
    border-radius: var(--radius-md);
    overflow: hidden;
    background: var(--surface-soft);
    border: 1px solid var(--border);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .tile-media {
    width: 100%;
    height: 100%;
    object-fit: contain;
    background: #000;
  }
  .mirrored {
    transform: scaleX(-1);
  }
  .tile-avatar {
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .tile-btn {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    background: var(--bg-primary);
    border: 1px solid var(--border);
    cursor: pointer;
    opacity: 0.55;
    transition:
      opacity var(--transition),
      color var(--transition);
  }
  @media (hover: hover) {
    .tile-btn:hover {
      opacity: 1;
      color: var(--text-primary);
    }
  }
  @supports (background: color-mix(in srgb, red 50%, transparent)) {
    .tile-btn {
      background: color-mix(in srgb, var(--bg-primary) 70%, transparent);
    }
  }
  .tile-name {
    position: absolute;
    left: 8px;
    bottom: 8px;
    padding: 2px 8px;
    border-radius: 999px;
    font-size: var(--text-2xs);
    color: var(--text-primary);
    background: var(--bg-primary);
  }
  @supports (background: color-mix(in srgb, red 50%, transparent)) {
    .tile-name {
      background: color-mix(in srgb, var(--bg-primary) 70%, transparent);
    }
  }
  @media (max-width: 560px) {
    .tiles {
      grid-template-columns: 1fr;
      --tile-rows: 2;
    }
    .tiles.side-by-side {
      grid-template-columns: 1fr 1fr;
      --tile-rows: 1;
    }
  }

  @media (max-height: 520px) {
    .stage.video {
      gap: 10px;
      padding-top: 38px;
    }
    .stage.video .name,
    .stage.video .trust,
    .stage.video .ctl-label {
      display: none;
    }
    .stage.video .round {
      width: 46px;
      height: 46px;
    }
    .stage.video .tiles {
      --call-chrome: calc(200px + var(--safe-top) + var(--safe-bottom));
    }
  }

  .identity {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    text-align: center;
  }
  .name {
    font-size: var(--text-2xl);
    font-weight: 600;
    color: var(--text-primary);
    margin: 0;
    max-width: 300px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .phase {
    font-family: var(--font-mono);
    font-size: var(--text-sm);
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.02em;
  }
  .phase.live {
    color: var(--accent-hover);
  }
  .voice-badge {
    position: absolute;
    bottom: 2px;
    right: 2px;
    z-index: 3;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    border-radius: 50%;
    background: var(--danger);
    color: #fff;
    border: 3px solid var(--bg-primary);
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.4);
  }
  .trust {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    margin-top: 4px;
    padding: 3px 10px;
    border-radius: 999px;
    font-size: var(--text-2xs);
    font-weight: 500;
    color: var(--warning);
    background: color-mix(in srgb, var(--warning) 14%, transparent);
  }
  .trust.ok {
    color: var(--success);
    background: color-mix(in srgb, var(--success) 14%, transparent);
  }

  .controls {
    display: flex;
    align-items: flex-start;
    justify-content: center;
    gap: 26px;
    margin-top: 8px;
  }
  .ctl-group {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }
  .ctl-label {
    font-size: var(--text-2xs);
    color: var(--text-muted);
  }
  .ctl-split {
    position: relative;
    display: flex;
  }
  .caret {
    position: absolute;
    right: -2px;
    bottom: -2px;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    background: var(--bg-tertiary);
    border: 1px solid var(--border);
    transition:
      color var(--transition),
      background var(--transition),
      transform var(--transition);
  }
  .caret.open {
    color: var(--text-primary);
    background: var(--surface-soft);
  }
  @media (hover: hover) {
    .caret:hover {
      color: var(--text-primary);
      background: var(--surface-soft);
    }
  }
  .caret.open {
    transform: rotate(180deg);
  }
  .cam-menu {
    position: absolute;
    bottom: calc(100% + 10px);
    left: 50%;
    transform: translateX(-50%);
    z-index: 4;
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: max-content;
    max-width: min(280px, 70vw);
    padding: 6px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    box-shadow: var(--shadow-3);
  }
  .menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    font-size: var(--text-sm);
    color: var(--text-secondary);
    text-align: left;
    transition:
      background var(--transition),
      color var(--transition);
  }
  @media (hover: hover) {
    .menu-item:hover {
      color: var(--text-primary);
      background: var(--bg-hover);
    }
  }
  .menu-item[aria-checked='true'] {
    color: var(--text-primary);
  }
  .menu-check {
    display: flex;
    flex: 0 0 13px;
    color: var(--accent-hover);
  }
  .menu-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .round {
    width: 56px;
    height: 56px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-primary);
    background: var(--surface-soft);
    border: 1px solid var(--border);
    box-shadow: 0 0 0 calc(var(--glow, 0) * 6px)
      color-mix(in srgb, var(--success) calc(var(--glow, 0) * 40%), transparent);
    transition:
      background var(--transition),
      color var(--transition),
      transform var(--transition);
  }
  @media (hover: hover) {
    .round:hover {
      transform: scale(1.05);
    }
  }
  .round:active {
    transform: scale(0.95);
  }
  .round:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .round:disabled:active {
    transform: none;
  }
  @media (hover: hover) {
    .round:disabled:hover {
      transform: none;
    }
  }
  .round.active {
    background: var(--accent-dim);
    color: var(--accent-hover);
    border-color: color-mix(in srgb, var(--accent) 40%, transparent);
  }
  .round.denied {
    color: var(--danger);
    border-color: color-mix(in srgb, var(--danger) 40%, transparent);
  }
  .round.decline {
    color: #fff;
    background: var(--danger);
    border-color: transparent;
  }
  @media (hover: hover) {
    .round.decline:hover {
      filter: brightness(1.08);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .avatar-wrap.ringing .ring,
    .ring {
      transition: none;
    }
  }
</style>
