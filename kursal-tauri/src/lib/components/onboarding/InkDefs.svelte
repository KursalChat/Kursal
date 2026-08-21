<svg class="ink-defs" aria-hidden="true" focusable="false">
  <defs>
    <filter id="ink-wobble" x="-6%" y="-6%" width="112%" height="112%">
      <feTurbulence type="fractalNoise" baseFrequency="0.018" numOctaves="2" seed="7" result="n" />
      <feDisplacementMap
        in="SourceGraphic"
        in2="n"
        scale="2.6"
        xChannelSelector="R"
        yChannelSelector="G"
      />
    </filter>

    <filter id="ink-wobble-soft" x="-6%" y="-6%" width="112%" height="112%">
      <feTurbulence type="fractalNoise" baseFrequency="0.026" numOctaves="2" seed="23" result="n" />
      <feDisplacementMap
        in="SourceGraphic"
        in2="n"
        scale="1.5"
        xChannelSelector="R"
        yChannelSelector="G"
      />
    </filter>

    <clipPath id="ink-envelope-clip">
      <rect x="-30" y="-19" width="60" height="38" rx="3" />
    </clipPath>
  </defs>
</svg>

<style>
  .ink-defs {
    position: absolute;
    width: 0;
    height: 0;
    overflow: hidden;
  }
</style>
