import type {
  ConnectionChangedPayload,
  ContactMeta,
  ContactResponse,
  LtcStatus,
  MessageResponse,
} from '$lib/types';

export type ConnectionStatus = ConnectionChangedPayload['status'];

export const MINUTE = 60_000;
export const HOUR = 60 * MINUTE;
export const DAY = 24 * HOUR;

export const ME_ID = '4b7f1c9e2d8a6b3f5e0c7a9d1b4e8f2a6c3d9e7b1f5a8c2e4d6b0f3a7c9e1d5b';
export const ME_PEER = '12D3KooWMockSelfPeer8x3sLkQ9vRtZ2nJpYhW4cF6gBdE1aU7m';
export const ME_NAME = 'Mock User';

export const ALICE = 'a11ce0000000000000000000000000000000000000000000000000000000a11c';
export const BOB = 'b0b0000000000000000000000000000000000000000000000000000000000b0b';
export const CHLOE = 'c410e00000000000000000000000000000000000000000000000000000c410e0';
export const DAN = 'da0000000000000000000000000000000000000000000000000000000000da00';

export const SAMPLE_LINES = [
  'hey, you around?',
  "yeah what's up",
  'did you see the new build?',
  'lol',
  'sounds good 👍',
  'can we move it to tomorrow?',
  "I'll send the file in a bit",
  'ok perfect',
  'wait, which one',
  'the second link, not the first',
  'haha fair enough',
  'running 5 min late sorry',
  'np',
  'this is a slightly longer message to see how wrapping behaves when a line does not fit on one row',
  '**bold**, _italic_ and `inline code`',
  'check https://kursal.chat',
  'spoiler: ||the butler did it||',
];

export interface MockDb {
  contacts: ContactResponse[];
  meta: Record<string, ContactMeta>;
  messages: Record<string, MessageResponse[]>;
  readCursor: Record<string, string>;
  connection: Record<string, ConnectionStatus>;
  ltc: LtcStatus | null;
}

const hex = (bytes: Uint8Array) =>
  Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');

export const secs = (ms: number) => Math.floor(ms / 1000);

export function messageId(ms: number): string {
  const time = Math.max(0, Math.floor(ms)).toString(16).padStart(12, '0');
  return time + hex(crypto.getRandomValues(new Uint8Array(10)));
}

export function randomHex(bytes: number): string {
  return hex(crypto.getRandomValues(new Uint8Array(bytes)));
}

export function pick<T>(items: T[], rand: () => number = Math.random): T {
  return items[Math.floor(rand() * items.length)];
}

function mulberry32(seed: number): () => number {
  let a = seed;
  return () => {
    a |= 0;
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

interface MessageInit extends Partial<MessageResponse> {
  contactId: string;
  direction: 'sent' | 'received';
  content: string;
  sentMs: number;
  receivedMs?: number;
}

export function makeMessage({ sentMs, receivedMs, ...init }: MessageInit): MessageResponse {
  return {
    id: messageId(sentMs),
    status: init.direction === 'sent' ? 'read' : 'delivered',
    timestamp: secs(sentMs),
    receivedTimestamp: secs(receivedMs ?? sentMs),
    replyTo: null,
    ...init,
  };
}

export function makeContact(
  userId: string,
  displayName: string,
  createdMs: number,
  extra: Partial<ContactResponse> = {}
): ContactResponse {
  return {
    userId,
    displayName,
    peerId: `12D3KooW${userId.slice(0, 12)}MockPeer${userId.slice(-8)}`,
    knownAddresses: ['/ip4/192.168.1.24/udp/4001/quic-v1'],
    verified: false,
    profileShared: true,
    blocked: false,
    createdAt: secs(createdMs),
    avatarPath: null,
    profileName: displayName,
    ...extra,
  };
}

export function makeMeta(contactId: string, extra: Partial<ContactMeta> = {}): ContactMeta {
  return {
    contactId,
    muted: false,
    lastSeenAt: null,
    lastMessageAt: null,
    alias: null,
    terminated: false,
    ...extra,
  };
}

export function seedDb(now: number): MockDb {
  const rand = mulberry32(7);
  const messages: Record<string, MessageResponse[]> = {
    [ALICE]: [],
    [BOB]: [],
    [CHLOE]: [],
    [DAN]: [],
  };
  const push = (init: MessageInit) => {
    const msg = makeMessage(init);
    messages[init.contactId].push(msg);
    return msg;
  };

  let t = now - 6 * DAY;
  while (t < now - 3 * HOUR) {
    push({
      contactId: ALICE,
      direction: rand() < 0.5 ? 'sent' : 'received',
      content: pick(SAMPLE_LINES, rand),
      sentMs: t,
    });
    t += 5 * MINUTE + rand() * 90 * MINUTE;
  }

  const question = push({
    contactId: ALICE,
    direction: 'received',
    content: 'can you review the PR before friday?',
    sentMs: now - 150 * MINUTE,
    pinned: true,
  });
  push({
    contactId: ALICE,
    direction: 'sent',
    content: 'sure, on it',
    sentMs: now - 148 * MINUTE,
    replyTo: question.id,
    reactions: [{ emoji: '❤️', userId: ALICE }],
  });
  push({
    contactId: ALICE,
    direction: 'received',
    content: '',
    sentMs: now - 120 * MINUTE,
    callDetails: { outcome: 'completed', durationMs: 754_000 },
  });
  push({
    contactId: ALICE,
    direction: 'received',
    content: 'here is the snippet:\n\n```ts\nconst answer = 42;\nconsole.log(answer);\n```',
    sentMs: now - 90 * MINUTE,
    edited: true,
  });
  push({
    contactId: ALICE,
    direction: 'received',
    content: 'design-v2.pdf',
    sentMs: now - 80 * MINUTE,
    fileDetails: {
      filename: 'design-v2.pdf',
      sizeBytes: 2_480_000,
      autodownloadPath: null,
    },
  });
  push({
    contactId: ALICE,
    direction: 'received',
    content: 'sent this while you were offline',
    sentMs: now - 60 * MINUTE,
    receivedMs: now - 5 * MINUTE,
  });
  push({
    contactId: ALICE,
    direction: 'sent',
    content: 'got it, thanks!',
    sentMs: now - 4 * MINUTE,
    status: 'delivered',
    reactions: [{ emoji: '👍', userId: ALICE }],
  });

  push({
    contactId: BOB,
    direction: 'sent',
    content: 'hey Bob',
    sentMs: now - 2 * DAY,
  });
  push({
    contactId: BOB,
    direction: 'received',
    content: 'hey!',
    sentMs: now - 2 * DAY + MINUTE,
  });
  const bobRead = push({
    contactId: BOB,
    direction: 'sent',
    content: 'dinner on saturday?',
    sentMs: now - 40 * MINUTE,
  });
  push({
    contactId: BOB,
    direction: 'received',
    content: 'yes!!',
    sentMs: now - 12 * MINUTE,
  });
  push({
    contactId: BOB,
    direction: 'received',
    content: 'where?',
    sentMs: now - 11 * MINUTE,
  });
  push({
    contactId: BOB,
    direction: 'received',
    content: 'the usual place?',
    sentMs: now - 10 * MINUTE,
  });

  push({
    contactId: CHLOE,
    direction: 'received',
    content: 'bye!',
    sentMs: now - 3 * DAY,
  });
  push({
    contactId: CHLOE,
    direction: 'sent',
    content: 'this one never made it',
    sentMs: now - 2 * HOUR,
    status: 'failed',
  });
  push({
    contactId: CHLOE,
    direction: 'sent',
    content: 'waiting in the offline mailbox',
    sentMs: now - 30 * MINUTE,
    status: 'queued',
  });

  push({
    contactId: DAN,
    direction: 'received',
    content: 'removing you, bye',
    sentMs: now - DAY,
  });

  const contacts = [
    makeContact(ALICE, 'Alice', now - 30 * DAY, { verified: true }),
    makeContact(BOB, 'Bob', now - 10 * DAY),
    makeContact(CHLOE, 'Chloe', now - 20 * DAY, { verified: true }),
    makeContact(DAN, 'Dan', now - 40 * DAY),
  ];

  const meta: Record<string, ContactMeta> = {};
  for (const c of contacts) {
    const last = messages[c.userId].at(-1);
    meta[c.userId] = makeMeta(c.userId, {
      lastMessageAt: last?.timestamp ?? null,
    });
  }
  meta[CHLOE].lastSeenAt = now - 2 * DAY;
  meta[DAN].terminated = true;

  const lastId = (id: string) => messages[id].at(-1)?.id ?? '';

  return {
    contacts,
    meta,
    messages,
    readCursor: {
      [ALICE]: lastId(ALICE),
      [BOB]: bobRead.id,
      [CHLOE]: lastId(CHLOE),
      [DAN]: lastId(DAN),
    },
    connection: {
      [ALICE]: 'direct',
      [BOB]: 'relay',
      [CHLOE]: 'disconnected',
      [DAN]: 'disconnected',
    },
    ltc: null,
  };
}
