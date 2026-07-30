import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { SharePayload } from '$lib/types';

const mocks = vi.hoisted(() => ({
  takePendingShares: vi.fn(),
  discardPendingShare: vi.fn(),
}));

vi.mock('$lib/api/share', () => mocks);

vi.mock('$lib/utils/log', () => ({
  log: { debug: vi.fn(), info: vi.fn(), warn: vi.fn(), error: vi.fn() },
}));

import { shareIntentState } from './shareIntent.svelte';

const payload = (id: string, filenames: string[] = ['photo.jpg']): SharePayload => ({
  id,
  files: filenames.map((filename) => ({
    path: `/tmp/kursal-shares/${id}/${filename}`,
    filename,
    sizeBytes: 12,
  })),
  text: null,
});

describe('shareIntentState', () => {
  beforeEach(() => {
    shareIntentState.reset();
    mocks.takePendingShares.mockReset();
    mocks.discardPendingShare.mockReset();
    mocks.discardPendingShare.mockResolvedValue(undefined);
  });

  it('starts with nothing pending', () => {
    expect(shareIntentState.head).toBeNull();
    expect(shareIntentState.awaitingTarget).toBe(false);
  });

  it('drain queues payloads and awaits a target', async () => {
    mocks.takePendingShares.mockResolvedValue([payload('a')]);

    await shareIntentState.drain();

    expect(shareIntentState.head?.id).toBe('a');
    expect(shareIntentState.awaitingTarget).toBe(true);
  });

  it('claim returns nothing until a contact is assigned', async () => {
    mocks.takePendingShares.mockResolvedValue([payload('a')]);
    await shareIntentState.drain();

    expect(shareIntentState.claim('c1')).toBeNull();
    expect(shareIntentState.head?.id).toBe('a');
  });

  it('claim hands the payload to the assigned contact only', async () => {
    mocks.takePendingShares.mockResolvedValue([payload('a')]);
    await shareIntentState.drain();
    shareIntentState.assign('c1');

    expect(shareIntentState.claim('c2')).toBeNull();
    expect(shareIntentState.head?.id).toBe('a');

    expect(shareIntentState.claim('c1')?.id).toBe('a');
    expect(shareIntentState.head).toBeNull();
    expect(shareIntentState.awaitingTarget).toBe(false);
  });

  it('a claimed payload is not handed out twice', async () => {
    mocks.takePendingShares.mockResolvedValue([payload('a')]);
    await shareIntentState.drain();
    shareIntentState.assign('c1');

    expect(shareIntentState.claim('c1')?.id).toBe('a');
    expect(shareIntentState.claim('c1')).toBeNull();
  });

  it('surfaces queued payloads one at a time', async () => {
    mocks.takePendingShares.mockResolvedValue([payload('a'), payload('b')]);
    await shareIntentState.drain();

    shareIntentState.assign('c1');
    expect(shareIntentState.claim('c1')?.id).toBe('a');

    expect(shareIntentState.head?.id).toBe('b');
    expect(shareIntentState.awaitingTarget).toBe(true);
  });

  it('assign is ignored when the queue is empty', () => {
    shareIntentState.assign('c1');
    expect(shareIntentState.claim('c1')).toBeNull();
  });

  it('discardHead deletes the staged files and advances the queue', async () => {
    mocks.takePendingShares.mockResolvedValue([payload('a'), payload('b')]);
    await shareIntentState.drain();

    await shareIntentState.discardHead();

    expect(mocks.discardPendingShare).toHaveBeenCalledWith('a');
    expect(shareIntentState.head?.id).toBe('b');
  });

  it('discardHead clears a stale assignment', async () => {
    mocks.takePendingShares.mockResolvedValue([payload('a'), payload('b')]);
    await shareIntentState.drain();
    shareIntentState.assign('c1');

    await shareIntentState.discardHead();

    expect(shareIntentState.claim('c1')).toBeNull();
    expect(shareIntentState.awaitingTarget).toBe(true);
  });

  it('discardHead is a no-op with nothing queued', async () => {
    await shareIntentState.discardHead();
    expect(mocks.discardPendingShare).not.toHaveBeenCalled();
  });

  it('release swallows a failing delete', async () => {
    mocks.discardPendingShare.mockRejectedValue(new Error('gone'));
    await expect(shareIntentState.release('a')).resolves.toBeUndefined();
  });

  it('a failing drain leaves the queue untouched', async () => {
    mocks.takePendingShares.mockResolvedValue([payload('a')]);
    await shareIntentState.drain();

    mocks.takePendingShares.mockRejectedValue(new Error('no drop-box'));
    await shareIntentState.drain();

    expect(shareIntentState.head?.id).toBe('a');
  });

  it('the native bridge drains and unregisters cleanly', async () => {
    mocks.takePendingShares.mockResolvedValue([payload('a')]);

    const stop = shareIntentState.listenForNative();
    expect(typeof window.__kursalOnShare).toBe('function');

    window.__kursalOnShare?.();
    await vi.waitFor(() => expect(shareIntentState.head?.id).toBe('a'));

    stop();
    expect(window.__kursalOnShare).toBeUndefined();
  });

  it('overlapping drains only read the drop-box once', async () => {
    let resolveFirst: (value: SharePayload[]) => void = () => {};
    mocks.takePendingShares.mockImplementation(
      () =>
        new Promise<SharePayload[]>((resolve) => {
          resolveFirst = resolve;
        })
    );

    const first = shareIntentState.drain();
    const second = shareIntentState.drain();
    resolveFirst([payload('a')]);
    await Promise.all([first, second]);

    expect(mocks.takePendingShares).toHaveBeenCalledTimes(1);
    expect(shareIntentState.head?.id).toBe('a');
  });
});
