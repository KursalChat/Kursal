import { log } from '$lib/utils/log';

export type SoundName = 'ringtone';

const SOURCES: Record<SoundName, string> = {
  ringtone: '/sounds/ringtone.mp3',
};

const VOLUMES: Partial<Record<SoundName, number>> = {};

type PlayOptions = { loop?: boolean; volume?: number };

const looping = new Map<SoundName, HTMLAudioElement>();

export function playSound(name: SoundName, opts: PlayOptions = {}) {
  if (opts.loop && looping.has(name)) return;

  const el = new Audio(SOURCES[name]);
  el.volume = opts.volume ?? VOLUMES[name] ?? 1;
  el.loop = opts.loop ?? false;
  if (el.loop) looping.set(name, el);

  el.play().catch((e) => {
    if (el.loop) looping.delete(name);
    log.warn(`Sound "${name}" failed to play`, e);
  });
}

export function stopSound(name: SoundName) {
  const el = looping.get(name);
  if (!el) return;
  looping.delete(name);
  el.pause();
  el.currentTime = 0;
}

export function stopAllSounds() {
  for (const name of [...looping.keys()]) stopSound(name);
}
