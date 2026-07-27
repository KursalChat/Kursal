import { describe, it, expect } from 'vitest';
import { insertInSentOrder } from './delayedStore';

const m = (id: string, ts: number) => ({ id, timestamp: ts }) as never;

describe('insertInSentOrder', () => {
  it('inserts by ascending timestamp and returns the index', () => {
    const list = [m('a', 10), m('b', 30)];
    const idx = insertInSentOrder(list, m('c', 20));
    expect(list.map((x: { id: string }) => x.id)).toEqual(['a', 'c', 'b']);
    expect(idx).toBe(1);
  });

  it('appends when newest and reports the tail index', () => {
    const list = [m('a', 10)];
    const idx = insertInSentOrder(list, m('b', 20));
    expect(idx).toBe(1);
    expect(list.length - 1).toBe(idx);
  });

  it('breaks timestamp ties by id', () => {
    const list = [m('a', 10), m('c', 10)];
    const idx = insertInSentOrder(list, m('b', 10));
    expect(list.map((x: { id: string }) => x.id)).toEqual(['a', 'b', 'c']);
    expect(idx).toBe(1);
  });
});
