<script lang="ts">
  import type { Snippet } from 'svelte';
  import { fly, fade, scale } from 'svelte/transition';
  import { cubicOut } from 'svelte/easing';

  let {
    img,
    alt,
    size = 96,
    children,
  }: { img: string; alt: string; size?: number; children: Snippet } = $props();

  const reducedMotion =
    typeof window !== 'undefined' && window.matchMedia('(prefers-reduced-motion: reduce)').matches;
</script>

<div class="card" style="--winston-size: {size}px" out:fade|global={{ duration: 200 }}>
  <div
    class="winston-wrap"
    in:fly|global={{
      y: reducedMotion ? 0 : 130,
      duration: reducedMotion ? 0 : 750,
      easing: cubicOut,
    }}
  >
    <img src={img} {alt} class="winston" />
    <div class="winston-shadow"></div>
  </div>

  <div
    class="bubble"
    in:scale|global={{
      delay: reducedMotion ? 0 : 550,
      duration: reducedMotion ? 0 : 360,
      start: 0.8,
      easing: cubicOut,
    }}
  >
    <div class="bubble-tail"></div>
    {@render children()}
  </div>
</div>

<style>
  .card {
    display: flex;
    align-items: flex-end;
    gap: 12px;
  }

  .winston-wrap {
    position: relative;
    flex-shrink: 0;
  }
  .winston {
    width: var(--winston-size);
    height: var(--winston-size);
    object-fit: contain;
    filter: drop-shadow(0 12px 24px rgba(0, 0, 0, 0.45));
    user-select: none;
    -webkit-user-drag: none;
    animation: float 3.4s ease-in-out infinite;
    animation-delay: 750ms;
  }
  @keyframes float {
    0%,
    100% {
      transform: translateY(0);
    }
    50% {
      transform: translateY(-6px);
    }
  }
  .winston-shadow {
    position: absolute;
    bottom: -4px;
    left: 50%;
    transform: translateX(-50%);
    width: calc(var(--winston-size) * 0.64);
    height: 9px;
    background: radial-gradient(ellipse at center, rgba(0, 0, 0, 0.35) 0%, transparent 70%);
    border-radius: 50%;
    animation: shadow-pulse 3.4s ease-in-out infinite;
    animation-delay: 750ms;
    pointer-events: none;
  }
  @keyframes shadow-pulse {
    0%,
    100% {
      transform: translateX(-50%) scaleX(1);
      opacity: 0.7;
    }
    50% {
      transform: translateX(-50%) scaleX(0.78);
      opacity: 0.5;
    }
  }

  .bubble {
    position: relative;
    transform-origin: 0 80%;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius-md);
    padding: 15px 16px 13px;
    box-shadow:
      var(--glow),
      0 0 0 1px rgba(255, 255, 255, 0.04) inset;
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    flex: 1;
    min-width: 0;
  }
  .bubble-tail {
    position: absolute;
    left: -7px;
    bottom: 25px;
    width: 14px;
    height: 14px;
    background: var(--surface);
    border-left: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    transform: rotate(45deg);
  }

  @media (max-width: 768px) {
    .winston {
      width: calc(var(--winston-size) * 0.8);
      height: calc(var(--winston-size) * 0.8);
    }
  }

  @media (max-width: 480px) {
    .card {
      flex-direction: column;
      align-items: center;
      gap: 0;
    }
    .bubble-tail {
      display: none;
    }
    .bubble {
      width: 100%;
    }
    .winston {
      width: calc(var(--winston-size) * 0.87);
      height: calc(var(--winston-size) * 0.87);
      margin-bottom: -10px;
    }
  }
</style>
