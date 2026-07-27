<script lang="ts">
  import { t } from '$lib/i18n';

  type Props = {
    onNext: () => void;
    onBack?: () => void;
  };

  let { onNext }: Props = $props();

  let showLine1 = $state(false);
  let showLine2 = $state(false);
  let showFinal = $state(false);
  let showButtons = $state(false);
  let showWinston = $state(false);

  const WINSTON_AT = 400;
  const LINE1_AT = 900;
  const LINE2_AT = LINE1_AT + 1600;
  const FINAL_AT = LINE2_AT + 1000;
  const BUTTONS_AT = FINAL_AT + 1600;

  $effect(() => {
    const timers = [
      setTimeout(() => (showWinston = true), WINSTON_AT),
      setTimeout(() => (showLine1 = true), LINE1_AT),
      setTimeout(() => (showLine2 = true), LINE2_AT),
      setTimeout(() => (showFinal = true), FINAL_AT),
      setTimeout(() => (showButtons = true), BUTTONS_AT),
    ];
    return () => timers.forEach(clearTimeout);
  });

  const devices = Array.from({ length: 11 }, (_, i) => i);

  // deterministic pseudo-random per-device offsets so packets don't sync up and stack;
  // wide delay range gives visible gaps between packets
  const PACKET_DELAYS = [1.2, 4.8, 0.3, 7.1, 2.6, 9.4, 3.9, 6.2, 0.9, 5.5, 8.3];
  const PACKET_DURS = [6.1, 7.4, 6.8, 5.9, 7.2, 6.3, 7.6, 6.5, 7.0, 5.7, 6.9];

  function charReveal(text: string): { ch: string; delay: number }[] {
    return text.split('').map((ch, i) => ({ ch, delay: i * 28 }));
  }
</script>

<div class="screen">
  <div class="scene" aria-hidden="true">
    <svg class="lines" viewBox="0 0 1000 720" preserveAspectRatio="xMidYMid meet">
      <defs>
        <linearGradient id="lineGrad" x1="0" y1="1" x2="0" y2="0">
          <stop offset="0%" stop-color="rgba(255, 180, 120, 0)" />
          <stop offset="40%" stop-color="rgba(255, 140, 90, 0.35)" />
          <stop offset="100%" stop-color="rgba(255, 90, 70, 0.75)" />
        </linearGradient>
        <linearGradient id="towerGrad" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="#1a1f32" />
          <stop offset="45%" stop-color="#11152a" />
          <stop offset="100%" stop-color="#06080f" />
        </linearGradient>
        <linearGradient id="hornGrad" x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stop-color="#0a0d18" />
          <stop offset="100%" stop-color="#1a1f32" />
        </linearGradient>
        <radialGradient id="eyeGlow" cx="0.5" cy="0.5">
          <stop offset="0%" stop-color="rgba(255, 140, 80, 0.55)" />
          <stop offset="60%" stop-color="rgba(255, 110, 60, 0.15)" />
          <stop offset="100%" stop-color="rgba(255, 110, 60, 0)" />
        </radialGradient>
        <radialGradient id="irisGrad" cx="0.5" cy="0.5">
          <stop offset="0%" stop-color="#fff0d0" />
          <stop offset="25%" stop-color="#ffb060" />
          <stop offset="60%" stop-color="#e04818" />
          <stop offset="100%" stop-color="#3a0604" />
        </radialGradient>
        <radialGradient id="baseGlow" cx="0.5" cy="0.5">
          <stop offset="0%" stop-color="rgba(255, 120, 70, 0.45)" />
          <stop offset="100%" stop-color="rgba(255, 120, 70, 0)" />
        </radialGradient>
      </defs>

      <!-- crown halo behind eye -->
      <ellipse class="eye-halo-svg" cx="500" cy="150" rx="280" ry="140" fill="url(#eyeGlow)" />

      <!-- data lines feed tower base -->
      {#each devices as i}
        {@const x = 80 + (i * 840) / (devices.length - 1)}
        <path
          d={`M ${x} 720 L 500 662`}
          fill="none"
          stroke="url(#lineGrad)"
          stroke-width="1.2"
          class="data-line"
          style="--i: {i};"
        />
      {/each}

      <!-- base glow where signals pour in -->
      <ellipse cx="500" cy="665" rx="130" ry="18" fill="url(#baseGlow)" />

      <!-- TOWER: tall Barad-dûr silhouette -->
      <g class="tower">
        <!-- ground shadow -->
        <ellipse cx="500" cy="680" rx="150" ry="10" fill="rgba(0,0,0,0.65)" />

        <!-- base platform -->
        <polygon
          points="380,678 620,678 608,656 392,656"
          fill="#08090f"
          stroke="rgba(255,130,80,0.25)"
          stroke-width="1"
        />
        <line
          x1="388"
          y1="667"
          x2="612"
          y2="667"
          stroke="rgba(255,130,80,0.18)"
          stroke-width="0.8"
        />

        <!-- main tapered body: wide base → narrow spire -->
        <path
          d="M 398 656
             L 398 560
             L 415 560
             L 415 440
             L 432 440
             L 432 310
             L 450 310
             L 450 190
             L 550 190
             L 550 310
             L 568 310
             L 568 440
             L 585 440
             L 585 560
             L 602 560
             L 602 656 Z"
          fill="url(#towerGrad)"
          stroke="rgba(255,130,80,0.35)"
          stroke-width="1.2"
        />

        <!-- side buttresses for mass -->
        <polygon
          points="398,560 380,600 380,656 398,656"
          fill="#0a0d18"
          stroke="rgba(255,130,80,0.2)"
          stroke-width="0.8"
        />
        <polygon
          points="602,560 620,600 620,656 602,656"
          fill="#0a0d18"
          stroke="rgba(255,130,80,0.2)"
          stroke-width="0.8"
        />

        <!-- step ring bands (sit just below each horizontal ledge) -->
        <line x1="398" y1="563" x2="602" y2="563" stroke="rgba(255,120,70,0.22)" stroke-width="1" />
        <line x1="415" y1="443" x2="585" y2="443" stroke="rgba(255,120,70,0.18)" stroke-width="1" />
        <line x1="432" y1="313" x2="568" y2="313" stroke="rgba(255,120,70,0.18)" stroke-width="1" />
        <line x1="450" y1="208" x2="550" y2="208" stroke="rgba(255,120,70,0.2)" stroke-width="1" />

        <!-- crenellation teeth - sit ON each ledge, symmetric both sides -->
        <g fill="#06080f" stroke="rgba(255,130,80,0.3)" stroke-width="0.6">
          <!-- lowest ledge y=560: left span 398-415, right span 585-602 -->
          <rect x="399" y="554" width="4" height="6" />
          <rect x="405" y="554" width="4" height="6" />
          <rect x="411" y="554" width="4" height="6" />
          <rect x="586" y="554" width="4" height="6" />
          <rect x="592" y="554" width="4" height="6" />
          <rect x="598" y="554" width="4" height="6" />
          <!-- mid ledge y=440: left span 415-432, right span 568-585 -->
          <rect x="416" y="434" width="4" height="6" />
          <rect x="422" y="434" width="4" height="6" />
          <rect x="428" y="434" width="4" height="6" />
          <rect x="569" y="434" width="4" height="6" />
          <rect x="575" y="434" width="4" height="6" />
          <rect x="581" y="434" width="4" height="6" />
          <!-- upper ledge y=310: left span 432-450, right span 550-568 -->
          <rect x="433" y="304" width="4" height="6" />
          <rect x="440" y="304" width="4" height="6" />
          <rect x="446" y="304" width="4" height="6" />
          <rect x="551" y="304" width="4" height="6" />
          <rect x="558" y="304" width="4" height="6" />
          <rect x="564" y="304" width="4" height="6" />
        </g>
        <!-- vertical seam -->
        <line x1="500" y1="190" x2="500" y2="656" stroke="rgba(255,120,70,0.12)" stroke-width="1" />

        <!-- side vertical ribs -->
        <line
          x1="440"
          y1="320"
          x2="440"
          y2="420"
          stroke="rgba(255,120,70,0.1)"
          stroke-width="0.8"
        />
        <line
          x1="560"
          y1="320"
          x2="560"
          y2="420"
          stroke="rgba(255,120,70,0.1)"
          stroke-width="0.8"
        />
        <line
          x1="425"
          y1="450"
          x2="425"
          y2="550"
          stroke="rgba(255,120,70,0.1)"
          stroke-width="0.8"
        />
        <line
          x1="575"
          y1="450"
          x2="575"
          y2="550"
          stroke="rgba(255,120,70,0.1)"
          stroke-width="0.8"
        />

        <!-- corner rivets -->
        <g fill="rgba(255,140,90,0.45)">
          <circle cx="418" cy="545" r="1.2" />
          <circle cx="582" cy="545" r="1.2" />
          <circle cx="435" cy="425" r="1.2" />
          <circle cx="565" cy="425" r="1.2" />
          <circle cx="453" cy="295" r="1.2" />
          <circle cx="547" cy="295" r="1.2" />
          <circle cx="418" cy="650" r="1.2" />
          <circle cx="582" cy="650" r="1.2" />
        </g>

        <!-- glowing slits (watch-windows) -->
        <g class="slits">
          <rect x="460" y="228" width="80" height="2.5" fill="rgba(255,120,70,0.6)" />
          <rect x="460" y="240" width="80" height="2.5" fill="rgba(255,120,70,0.3)" />
          <!-- single slit rows retained at tier transitions for rhythm -->
          <rect x="442" y="420" width="116" height="2" fill="rgba(255,120,70,0.35)" />
          <rect x="422" y="548" width="156" height="2" fill="rgba(255,120,70,0.35)" />
          <rect x="405" y="578" width="190" height="2.5" fill="rgba(255,120,70,0.45)" />
          <rect x="405" y="590" width="190" height="2.5" fill="rgba(255,120,70,0.3)" />
          <rect x="405" y="602" width="190" height="2.5" fill="rgba(255,120,70,0.55)" />
          <rect x="405" y="620" width="190" height="2.5" fill="rgba(255,120,70,0.3)" />
        </g>

        <!-- VARIED INNER DETAILS -->
        <!-- spire rune glyphs -->
        <g fill="rgba(255,130,80,0.55)" class="runes">
          <rect x="470" y="258" width="3" height="3" />
          <rect x="476" y="258" width="2" height="3" />
          <rect x="481" y="258" width="4" height="3" />
          <rect x="488" y="258" width="2" height="3" />
          <rect x="493" y="258" width="3" height="3" />
          <rect x="500" y="258" width="3" height="3" />
          <rect x="506" y="258" width="2" height="3" />
          <rect x="511" y="258" width="4" height="3" />
          <rect x="518" y="258" width="2" height="3" />
          <rect x="523" y="258" width="3" height="3" />
        </g>

        <!-- spire small center porthole -->
        <circle
          cx="500"
          cy="280"
          r="3"
          fill="rgba(255,120,70,0.3)"
          stroke="rgba(255,140,90,0.65)"
          stroke-width="0.7"
        />
        <circle cx="500" cy="280" r="1" fill="rgba(255,220,170,0.85)" />

        <!-- mid-narrow tier: 3 arched windows -->
        <g stroke="rgba(255,130,80,0.55)" stroke-width="0.8" fill="rgba(255,120,70,0.22)">
          <path d="M 447 405 L 447 388 Q 447 378 455 378 Q 463 378 463 388 L 463 405 Z" />
          <path d="M 492 405 L 492 385 Q 492 373 500 373 Q 508 373 508 385 L 508 405 Z" />
          <path d="M 537 405 L 537 388 Q 537 378 545 378 Q 553 378 553 388 L 553 405 Z" />
        </g>

        <!-- mid-narrow tier X-bracing (diagonal beams) -->
        <g stroke="rgba(255,120,70,0.2)" stroke-width="0.7" fill="none">
          <line x1="440" y1="330" x2="560" y2="360" />
          <line x1="560" y1="330" x2="440" y2="360" />
          <circle cx="500" cy="345" r="1.5" fill="rgba(255,140,90,0.55)" stroke="none" />
        </g>

        <!-- machinery ring / cog at tier transition -->
        <g class="cog">
          <circle
            cx="500"
            cy="440"
            r="6"
            fill="none"
            stroke="rgba(255,130,80,0.5)"
            stroke-width="0.9"
          />
          <circle cx="500" cy="440" r="2.2" fill="rgba(255,120,70,0.6)" />
          <g stroke="rgba(255,130,80,0.5)" stroke-width="0.8">
            <line x1="494" y1="440" x2="491" y2="440" />
            <line x1="506" y1="440" x2="509" y2="440" />
            <line x1="500" y1="434" x2="500" y2="431" />
            <line x1="500" y1="446" x2="500" y2="449" />
          </g>
        </g>

        <!-- mid tier: 3 round portholes with rims -->
        <g>
          <circle
            cx="450"
            cy="495"
            r="5"
            fill="rgba(255,120,70,0.28)"
            stroke="rgba(255,140,90,0.7)"
            stroke-width="0.9"
          />
          <circle cx="450" cy="495" r="1.8" fill="rgba(255,210,160,0.85)" />
          <circle
            cx="500"
            cy="495"
            r="5"
            fill="rgba(255,120,70,0.22)"
            stroke="rgba(255,140,90,0.6)"
            stroke-width="0.9"
          />
          <circle cx="500" cy="495" r="1.8" fill="rgba(255,210,160,0.7)" />
          <circle
            cx="550"
            cy="495"
            r="5"
            fill="rgba(255,120,70,0.28)"
            stroke="rgba(255,140,90,0.7)"
            stroke-width="0.9"
          />
          <circle cx="550" cy="495" r="1.8" fill="rgba(255,210,160,0.85)" />
        </g>

        <!-- mid tier: internal ledge platform -->
        <rect
          x="430"
          y="520"
          width="140"
          height="3"
          fill="#0a0d18"
          stroke="rgba(255,130,80,0.4)"
          stroke-width="0.6"
        />
        <line
          x1="430"
          y1="520"
          x2="570"
          y2="520"
          stroke="rgba(255,140,90,0.35)"
          stroke-width="0.5"
        />
        <!-- railing posts on ledge -->
        <g stroke="rgba(255,130,80,0.45)" stroke-width="0.5">
          <line x1="445" y1="515" x2="445" y2="520" />
          <line x1="465" y1="515" x2="465" y2="520" />
          <line x1="485" y1="515" x2="485" y2="520" />
          <line x1="515" y1="515" x2="515" y2="520" />
          <line x1="535" y1="515" x2="535" y2="520" />
          <line x1="555" y1="515" x2="555" y2="520" />
        </g>

        <!-- base tier: heavy bolts around gate -->
        <g fill="rgba(255,140,90,0.55)">
          <circle cx="475" cy="625" r="1.4" />
          <circle cx="525" cy="625" r="1.4" />
          <circle cx="470" cy="645" r="1.4" />
          <circle cx="530" cy="645" r="1.4" />
          <circle cx="415" cy="605" r="1.3" />
          <circle cx="585" cy="605" r="1.3" />
          <circle cx="415" cy="635" r="1.3" />
          <circle cx="585" cy="635" r="1.3" />
        </g>

        <!-- vertical pipe/conduits on base sides -->
        <g stroke="rgba(255,130,80,0.35)" stroke-width="0.8" fill="none">
          <line x1="407" y1="575" x2="407" y2="650" />
          <line x1="593" y1="575" x2="593" y2="650" />
        </g>
        <g fill="rgba(255,140,90,0.4)">
          <circle cx="407" cy="590" r="1.1" />
          <circle cx="407" cy="615" r="1.1" />
          <circle cx="407" cy="640" r="1.1" />
          <circle cx="593" cy="590" r="1.1" />
          <circle cx="593" cy="615" r="1.1" />
          <circle cx="593" cy="640" r="1.1" />
        </g>

        <!-- arched gate at base -->
        <path
          d="M 485 656 L 485 628 Q 485 615 500 615 Q 515 615 515 628 L 515 656 Z"
          fill="#020305"
          stroke="rgba(255,130,80,0.45)"
          stroke-width="1"
        />
        <path
          d="M 489 654 L 489 628 Q 489 619 500 619 Q 511 619 511 628 L 511 654 Z"
          fill="rgba(255,120,70,0.06)"
        />
        <circle cx="500" cy="636" r="1.5" fill="rgba(255,140,90,0.55)" />

        <!-- crown horns framing the eye -->
        <path
          d="M 450 190 Q 430 110 415 40 Q 438 110 470 190 Z"
          fill="url(#hornGrad)"
          stroke="rgba(255,130,80,0.45)"
          stroke-width="1"
        />
        <path
          d="M 465 190 Q 468 130 478 80 Q 480 135 485 190 Z"
          fill="url(#hornGrad)"
          stroke="rgba(255,130,80,0.4)"
          stroke-width="1"
        />
        <path
          d="M 550 190 Q 570 110 585 40 Q 562 110 530 190 Z"
          fill="url(#hornGrad)"
          stroke="rgba(255,130,80,0.45)"
          stroke-width="1"
        />
        <path
          d="M 535 190 Q 532 130 522 80 Q 520 135 515 190 Z"
          fill="url(#hornGrad)"
          stroke="rgba(255,130,80,0.4)"
          stroke-width="1"
        />

        <!-- eye socket cradle between horns -->
        <ellipse
          cx="500"
          cy="148"
          rx="46"
          ry="24"
          fill="#05060b"
          stroke="rgba(255,130,80,0.5)"
          stroke-width="1.2"
        />

        <!-- THE EYE -->
        <ellipse class="rays" cx="500" cy="148" rx="60" ry="32" fill="url(#eyeGlow)" />
        <ellipse class="iris" cx="500" cy="148" rx="38" ry="18" fill="url(#irisGrad)" />
        <ellipse class="pupil" cx="500" cy="148" rx="3.2" ry="14" fill="#05020a" />
      </g>

      <!-- packets travel from devices UP through tower into the eye (rendered on top) -->
      {#each devices as i}
        {@const x = 80 + (i * 840) / (devices.length - 1)}
        <circle
          r="2.6"
          class="packet"
          style="
            --delay: {PACKET_DELAYS[i]}s;
            --dur: {PACKET_DURS[i]}s;
            offset-path: path('M {x} 720 L 500 662 L 500 148');
          "
        />
      {/each}
    </svg>

    {#if showWinston}
      <div class="winston-corner">
        <img src="/winston.png" alt="" draggable="false" />
        <div class="winston-glow"></div>
      </div>
    {/if}
  </div>

  <div class="text-block">
    <div class="line large" class:show={showLine1}>
      {#if showLine1}
        {#each charReveal(t('onboarding.screen2.line1')) as { ch, delay }}
          <span class="char" style="animation-delay: {delay}ms">{ch === ' ' ? '\u00A0' : ch}</span>
        {/each}
      {/if}
    </div>

    <div class="line" class:show={showLine2}>
      {#if showLine2}
        {#each charReveal(t('onboarding.screen2.line2')) as { ch, delay }}
          <span class="char" style="animation-delay: {delay}ms">{ch === ' ' ? '\u00A0' : ch}</span>
        {/each}
      {/if}
    </div>

    <div class="line final" class:show={showFinal}>
      {#if showFinal}
        {#each charReveal(t('onboarding.screen2.line3')) as { ch, delay }}
          <span class="char" style="animation-delay: {delay * 1.8}ms"
            >{ch === ' ' ? '\u00A0' : ch}</span
          >
        {/each}
      {/if}
    </div>
  </div>

  <div class="actions" class:show={showButtons}>
    <button class="next" onclick={onNext} tabindex={showButtons ? 0 : -1}>
      <span>{t('onboarding.screen2.cta')}</span>
      <svg
        width="18"
        height="18"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        stroke-width="2.5"
        stroke-linecap="round"
        stroke-linejoin="round"><path d="M5 12h14M13 6l6 6-6 6" /></svg
      >
    </button>
  </div>
</div>

<style>
  .screen {
    width: 100%;
    min-height: 100%;
    display: grid;
    grid-template-rows: 1fr auto auto;
    align-items: center;
    justify-items: center;
    gap: 20px;
    padding: 32px 40px 40px;
    position: relative;
  }

  /* SCENE */
  .scene {
    position: relative;
    width: min(820px, 100%);
    aspect-ratio: 1000 / 720;
    max-height: 58vh;
    animation: sceneFade 900ms ease-out 80ms both;
  }

  @keyframes sceneFade {
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .lines {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .data-line {
    stroke-dasharray: 4 6;
    opacity: 0;
    animation:
      lineIn 600ms ease-out calc(150ms + var(--i) * 35ms) forwards,
      dashFlow 8s linear calc(150ms + var(--i) * 35ms) infinite;
  }

  @keyframes lineIn {
    from {
      opacity: 0;
      stroke-dashoffset: 200;
    }
    to {
      opacity: 0.7;
      stroke-dashoffset: 0;
    }
  }

  @keyframes dashFlow {
    to {
      stroke-dashoffset: -200;
    }
  }

  .packet {
    fill: #ffb07a;
    filter: drop-shadow(0 0 5px rgba(255, 140, 80, 1));
    offset-distance: 0%;
    opacity: 0;
    animation: packetFlow var(--dur, 6s) linear var(--delay, 0s) infinite backwards;
  }

  @keyframes packetFlow {
    0% {
      offset-distance: 0%;
      opacity: 0;
    }
    8% {
      opacity: 1;
    }
    70% {
      opacity: 1;
    }
    90% {
      opacity: 0;
    }
    100% {
      offset-distance: 100%;
      opacity: 0;
    }
  }

  /* TOWER */
  .tower {
    opacity: 0;
    transform: translateY(18px);
    transform-origin: 500px 680px;
    animation: towerIn 1100ms cubic-bezier(0.22, 1, 0.36, 1) 300ms forwards;
  }
  @keyframes towerIn {
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .slits rect {
    filter: drop-shadow(0 0 3px rgba(255, 120, 70, 0.7));
    animation: slitsFlicker 4.5s ease-in-out infinite;
  }
  @keyframes slitsFlicker {
    0%,
    100% {
      opacity: 1;
    }
    47% {
      opacity: 0.9;
    }
    50% {
      opacity: 0.4;
    }
    53% {
      opacity: 0.95;
    }
  }

  /* EYE - use fill-box with center origin so transforms stay anchored to each ellipse */
  .eye-halo-svg {
    transform-box: fill-box;
    transform-origin: center;
    animation: haloPulse 3.8s ease-in-out infinite;
  }
  @keyframes haloPulse {
    0%,
    100% {
      opacity: 0.85;
      transform: scale(1);
    }
    50% {
      opacity: 1;
      transform: scale(1.08);
    }
  }

  .iris {
    transform-box: fill-box;
    transform-origin: center;
    filter: drop-shadow(0 0 12px rgba(255, 120, 70, 0.6));
    animation: blinkEye 5.4s ease-in-out infinite;
  }
  @keyframes blinkEye {
    0%,
    94%,
    100% {
      transform: scaleY(1);
    }
    96% {
      transform: scaleY(0.08);
    }
    98% {
      transform: scaleY(1);
    }
  }

  .pupil {
    transform-box: fill-box;
    transform-origin: center;
    animation: pupilShift 6s ease-in-out infinite;
  }
  @keyframes pupilShift {
    0%,
    100% {
      transform: translate(0, 0);
    }
    30% {
      transform: translate(6px, 0);
    }
    60% {
      transform: translate(-7px, 0);
    }
  }

  .rays {
    transform-box: fill-box;
    transform-origin: center;
    animation: raysPulse 2.8s ease-in-out infinite;
    pointer-events: none;
  }
  @keyframes raysPulse {
    0%,
    100% {
      opacity: 0.7;
      transform: scale(1);
    }
    50% {
      opacity: 1;
      transform: scale(1.18);
    }
  }

  /* WINSTON */
  .winston-corner {
    position: absolute;
    bottom: 36px;
    left: 12px;
    width: 78px;
    height: 78px;
    transform: rotate(-8deg);
    animation:
      winstonIn 800ms cubic-bezier(0.22, 1, 0.36, 1) backwards,
      nervous 3.4s ease-in-out 800ms infinite;
    opacity: 0.9;
  }

  .winston-corner img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    filter: drop-shadow(0 0 10px rgba(46, 91, 215, 0.22)) brightness(0.88);
    user-select: none;
    -webkit-user-drag: none;
  }

  .winston-glow {
    position: absolute;
    inset: 10%;
    background: radial-gradient(circle, rgba(46, 91, 215, 0.2) 0%, transparent 70%);
    filter: blur(12px);
    z-index: -1;
  }

  @keyframes winstonIn {
    from {
      opacity: 0;
      transform: translateY(10px) rotate(-8deg) scale(0.9);
    }
    to {
      opacity: 0.9;
      transform: translateY(0) rotate(-8deg) scale(1);
    }
  }

  @keyframes nervous {
    0%,
    100% {
      transform: translateX(0) rotate(-8deg);
    }
    25% {
      transform: translateX(-2px) rotate(-11deg);
    }
    50% {
      transform: translateX(1px) rotate(-6deg);
    }
    75% {
      transform: translateX(-1px) rotate(-10deg);
    }
  }

  /* TEXT */
  .text-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    text-align: center;
    max-width: 720px;
    min-height: 140px;
  }

  .line {
    font-size: 16px;
    color: rgba(200, 215, 255, 0.72);
    letter-spacing: 0.005em;
    min-height: 22px;
  }

  .line.large {
    font-size: 22px;
    font-weight: 600;
    color: #e8eeff;
    letter-spacing: -0.01em;
  }

  .line.final {
    font-size: 20px;
    font-weight: 600;
    color: rgba(255, 235, 220, 0.9);
    font-style: italic;
    letter-spacing: 0.01em;
    margin-top: 2px;
  }

  .char {
    display: inline-block;
    opacity: 0;
    transform: translateY(6px);
    animation: charIn 600ms ease-out forwards;
  }

  @keyframes charIn {
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  /* ACTIONS */
  .actions {
    display: flex;
    align-items: center;
    gap: 12px;
    min-height: 52px;
    opacity: 0;
    pointer-events: none;
    transition: opacity 500ms ease;
  }
  .actions.show {
    opacity: 1;
    pointer-events: auto;
  }

  .next {
    display: inline-flex;
    align-items: center;
    gap: 10px;
    padding: 12px 26px;
    background: linear-gradient(135deg, #2e5bd7 0%, #1e50e5 100%);
    color: #fff;
    border-radius: var(--radius-md);
    font-size: 15px;
    font-weight: 600;
    letter-spacing: 0.01em;
    box-shadow:
      0 0 0 1px rgba(123, 163, 247, 0.4),
      0 0 24px rgba(46, 91, 215, 0.55),
      0 8px 24px rgba(0, 0, 0, 0.4);
    transition:
      transform 150ms ease,
      box-shadow 150ms ease;
  }

  .next:hover {
    transform: translateY(-1px);
    box-shadow:
      0 0 0 1px rgba(123, 163, 247, 0.6),
      0 0 32px rgba(46, 91, 215, 0.75),
      0 10px 28px rgba(0, 0, 0, 0.5);
  }
  .next:active {
    transform: translateY(0);
  }

  @media (max-width: 640px) {
    .screen {
      gap: 14px;
      padding: 16px 16px 24px;
    }
    .line.large {
      font-size: 18px;
    }
    .line {
      font-size: 14px;
    }
    .line.final {
      font-size: 16px;
    }
    .text-block {
      min-height: 120px;
      gap: 6px;
    }
    .winston-corner {
      width: 62px;
      height: 62px;
      bottom: 18px;
      left: 6px;
    }
  }
</style>
