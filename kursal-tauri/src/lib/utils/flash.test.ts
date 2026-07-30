import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flash, flashSet } from './flash.svelte';

describe('flash', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('starts inactive', () => {
    expect(flash().active).toBe(false);
  });

  it('activates on trigger and clears after the duration', () => {
    const f = flash(1000);
    f.trigger();
    expect(f.active).toBe(true);
    vi.advanceTimersByTime(999);
    expect(f.active).toBe(true);
    vi.advanceTimersByTime(1);
    expect(f.active).toBe(false);
  });

  it('restarts the window when retriggered', () => {
    const f = flash(1000);
    f.trigger();
    vi.advanceTimersByTime(800);
    f.trigger();
    vi.advanceTimersByTime(800);
    expect(f.active).toBe(true);
    vi.advanceTimersByTime(200);
    expect(f.active).toBe(false);
  });
});

describe('flashSet', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('tracks keys independently', () => {
    const f = flashSet(1000);
    f.trigger('a');
    vi.advanceTimersByTime(600);
    f.trigger('b');
    expect(f.has('a')).toBe(true);
    expect(f.has('b')).toBe(true);
    vi.advanceTimersByTime(400);
    expect(f.has('a')).toBe(false);
    expect(f.has('b')).toBe(true);
    vi.advanceTimersByTime(600);
    expect(f.has('b')).toBe(false);
  });

  it('reports unknown keys as inactive', () => {
    expect(flashSet().has('nope')).toBe(false);
  });
});
