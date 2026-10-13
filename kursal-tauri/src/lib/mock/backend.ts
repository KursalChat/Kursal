import { emit } from '@tauri-apps/api/event';
import type { CallStatus, ContactResponse, MessageResponse, UnreadEntry } from '#lib/types.js';
import {
  DAY,
  HOUR,
  ME_ID,
  ME_NAME,
  ME_PEER,
  SAMPLE_LINES,
  makeContact,
  makeMessage,
  makeMeta,
  pick,
  randomHex,
  secs,
  seedDb,
  type ConnectionStatus,
} from './fixtures';

interface Args {
  contactId?: string;
  messageId?: string;
  messageIds?: string[];
  fromMessageId?: string | null;
  text?: string;
  newContent?: string;
  replyTo?: string | null;
  limit?: number;
  before?: string | null;
  after?: string;
  query?: string;
  emoji?: string;
  pinned?: boolean;
  offerId?: string;
  savePath?: string;
  filePath?: string;
  filename?: string;
  callId?: string;
  key?: string;
  value?: unknown;
  paths?: string[];
  words?: string[];
  maxUses?: number | null;
  ttlSecs?: number | null;
  enabled?: boolean;
  url?: string;
}

type Timer = ReturnType<typeof setTimeout>;

export const MOCK_OS_KEY = 'kursal_mock_os';
export const controls = { latencyMs: 0, autoReply: true };

const db = seedDb(Date.now());
const uiState = new Map<string, string>();
const markedUnread = new Set<string>();
const transfers = new Map<string, Timer>();
const calls = new Map<string, { connectedAt: number | null; timers: Timer[] }>();
const warned = new Set<string>();
let statsTimer: Timer | null = null;

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function fire(event: string, payload: unknown, delayMs = 0) {
  setTimeout(() => void emit(event, structuredClone(payload)), delayMs);
}

function warnOnce(cmd: string) {
  if (warned.has(cmd)) return;
  warned.add(cmd);
  console.warn(`[mock] unhandled command: ${cmd}`);
}

function list(contactId: string): MessageResponse[] {
  return (db.messages[contactId] ??= []);
}

function find(contactId: string, id: string | undefined) {
  return list(contactId).find((m) => m.id === id);
}

function insert(msg: MessageResponse) {
  const l = list(msg.contactId);
  let i = l.length;
  while (i > 0 && l[i - 1].id > msg.id) i--;
  l.splice(i, 0, msg);
  const meta = db.meta[msg.contactId];
  if (meta) meta.lastMessageAt = Math.max(meta.lastMessageAt ?? 0, msg.timestamp);
}

function remove(contactId: string, id: string | undefined) {
  const l = list(contactId);
  const i = l.findIndex((m) => m.id === id);
  if (i >= 0) l.splice(i, 1);
}

function contact(contactId: string) {
  return db.contacts.find((c) => c.userId === contactId);
}

function isReachable(contactId: string) {
  const status = db.connection[contactId];
  return !!status && status !== 'disconnected' && !db.meta[contactId]?.terminated;
}

function unreadIds(contactId: string) {
  const cursor = db.readCursor[contactId] ?? '';
  return list(contactId)
    .filter((m) => m.direction === 'received' && !m.callDetails && m.id > cursor)
    .map((m) => m.id);
}

function unreadEntry(contactId: string): UnreadEntry {
  const ids = unreadIds(contactId);
  return {
    contactId,
    count: ids.length,
    capped: false,
    firstUnread: ids[0] ?? null,
    markedUnread: markedUnread.has(contactId),
  };
}

function lastWhere(contactId: string, test: (m: MessageResponse) => boolean) {
  return list(contactId).findLast(test);
}

function addContact(name: string, viaNearby: boolean): ContactResponse {
  const created = makeContact(randomHex(32), name, Date.now());
  db.contacts.push(created);
  db.meta[created.userId] = makeMeta(created.userId);
  db.connection[created.userId] = 'direct';
  fire('contact_added', { ...created, viaNearby }, 200);
  fire('connection_changed', { contactId: created.userId, status: 'direct' }, 400);
  return created;
}

function sendText(contactId: string, text: string, replyTo: string | null) {
  const reachable = isReachable(contactId);
  const msg = makeMessage({
    contactId,
    direction: 'sent',
    content: text,
    sentMs: Date.now(),
    status: reachable ? 'delivered' : 'queued',
    replyTo,
  });
  insert(msg);
  if (!reachable) {
    fire('message_queued_offline', { contactId, messageId: msg.id }, 80);
    return msg.id;
  }
  fire('delivery_confirmed', { contactId, messageId: msg.id }, 150);
  if (controls.autoReply) {
    fire('typing_indicator', { contactId, replyTo: null }, 500);
    setTimeout(() => {
      msg.status = 'read';
      fire('messages_read', { contactId, messageIds: [msg.id] });
      peer.message(contactId);
    }, 1800);
  }
  return msg.id;
}

function simulateDownload(contactId: string, offerId: string, savePath: string) {
  const msg = find(contactId, offerId);
  const total = msg?.fileDetails?.sizeBytes ?? 1_000_000;
  let sent = 0;
  const tick = () => {
    sent = Math.min(total, sent + Math.ceil(total / 12));
    fire('file_transfer_progress', {
      transferId: offerId,
      bytesTransferred: sent,
      totalBytes: total,
    });
    if (sent < total) {
      transfers.set(offerId, setTimeout(tick, 200));
      return;
    }
    transfers.delete(offerId);
    if (msg?.fileDetails) msg.fileDetails.autodownloadPath = savePath;
    fire('file_received', { contactId, transferId: offerId, savePath }, 50);
  };
  transfers.set(offerId, setTimeout(tick, 200));
}

function callSteps(callId: string, steps: [CallStatus, number][]) {
  const call = calls.get(callId);
  if (!call) return;
  for (const [state, delayMs] of steps) {
    call.timers.push(
      setTimeout(() => {
        if (state === 'connected') call.connectedAt = Date.now();
        void emit('call_state', { callId, state });
      }, delayMs)
    );
  }
  call.timers.push(
    setInterval(() => {
      if (call.connectedAt) void emit('call_level', { mic: Math.random(), remote: Math.random() });
    }, 120)
  );
}

function endCall(callId: string, reason: string) {
  const call = calls.get(callId);
  if (!call) return;
  call.timers.forEach((timer) => clearTimeout(timer));
  calls.delete(callId);
  const durationMs = call.connectedAt ? Date.now() - call.connectedAt : 0;
  fire('call_ended', { callId, reason, durationMs });
}

function nodeStats() {
  return {
    cpuPercent: 2 + Math.random() * 6,
    memBytes: 180_000_000 + Math.random() * 20_000_000,
    uptimeSecs: Math.floor(performance.now() / 1000),
    bytesIn: 48_000_000,
    bytesOut: 21_000_000,
    rateIn: Math.random() * 40_000,
    rateOut: Math.random() * 20_000,
  };
}

function command(cmd: string, a: Args): unknown {
  const cid = a.contactId ?? '';
  switch (cmd) {
    case 'frontend_ready':
      for (const [contactId, status] of Object.entries(db.connection)) {
        fire('connection_changed', { contactId, status }, 50);
      }
      fire('network_online', { online: true, peerCount: 3 }, 50);
      return null;

    case 'get_local_peer_id':
      return ME_PEER;
    case 'get_local_user_id_hex':
      return ME_ID;
    case 'get_local_user_profile':
      return [ME_NAME, null];

    case 'get_contacts':
      return db.contacts;
    case 'get_contact_meta':
      return Object.values(db.meta);
    case 'get_security_code':
      return 'hello this is a demo code and wish you a good day';
    case 'confirm_security_code': {
      const c = contact(cid);
      if (c) {
        c.verified = true;
        fire('contact_updated', c);
      }
      return null;
    }
    case 'set_contact_blocked': {
      const c = contact(cid);
      if (c) c.blocked = a.value === true;
      return null;
    }
    case 'set_contact_muted':
      if (db.meta[cid]) db.meta[cid].muted = a.value === true;
      return null;
    case 'set_contact_alias':
      if (db.meta[cid]) db.meta[cid].alias = (a.value as string | null) || null;
      return null;
    case 'remove_contact':
      db.contacts = db.contacts.filter((c) => c.userId !== cid);
      delete db.meta[cid];
      delete db.messages[cid];
      delete db.connection[cid];
      return null;

    case 'get_messages': {
      const l = list(cid);
      const at = a.before ? l.findIndex((m) => m.id === a.before) : -1;
      const end = at >= 0 ? at : l.length;
      return l.slice(Math.max(0, end - (a.limit ?? 50)), end);
    }
    case 'get_messages_after': {
      const l = list(cid);
      const at = l.findIndex((m) => m.id === a.after);
      return l.slice(at + 1, at + 1 + (a.limit ?? 50));
    }
    case 'get_messages_around': {
      const l = list(cid);
      const at = Math.max(
        0,
        l.findIndex((m) => m.id === a.messageId)
      );
      const half = Math.floor((a.limit ?? 50) / 2);
      return l.slice(Math.max(0, at - half), at + half + 1);
    }
    case 'search_messages':
    case 'search_messages_global': {
      const q = (a.query ?? '').toLowerCase();
      const pool = cmd === 'search_messages' ? list(cid) : Object.values(db.messages).flat();
      return pool
        .filter((m) => m.content.toLowerCase().includes(q))
        .sort((x, y) => (x.id < y.id ? 1 : -1))
        .slice(0, a.limit ?? 50);
    }
    case 'get_pinned_messages':
      return list(cid).filter((m) => m.pinned);

    case 'send_text':
      return sendText(cid, a.text ?? '', a.replyTo ?? null);
    case 'retry_message': {
      const msg = find(cid, a.messageId);
      if (msg && isReachable(cid)) {
        msg.status = 'delivered';
        fire('delivery_confirmed', { contactId: cid, messageId: msg.id }, 300);
      }
      return null;
    }
    case 'edit_message': {
      const msg = find(cid, a.messageId);
      if (msg) {
        msg.content = a.newContent ?? msg.content;
        msg.edited = true;
      }
      return false;
    }
    case 'delete_local_message':
    case 'delete_message_for_everyone':
      remove(cid, a.messageId);
      return cmd === 'delete_local_message' ? null : false;
    case 'add_reaction': {
      const msg = find(cid, a.messageId);
      if (msg && a.emoji)
        msg.reactions = [...(msg.reactions ?? []), { emoji: a.emoji, userId: ME_ID }];
      return false;
    }
    case 'remove_reaction': {
      const msg = find(cid, a.messageId);
      if (msg) {
        msg.reactions = msg.reactions?.filter((r) => !(r.emoji === a.emoji && r.userId === ME_ID));
      }
      return false;
    }
    case 'pin_message': {
      const msg = find(cid, a.messageId);
      if (msg) msg.pinned = a.pinned === true;
      return null;
    }
    case 'clear_message_history':
      if (cid) db.messages[cid] = [];
      else db.messages = {};
      return null;

    case 'get_unread_summary':
      return db.contacts.map((c) => unreadEntry(c.userId));
    case 'mark_contact_read': {
      const ids = unreadIds(cid);
      db.readCursor[cid] = list(cid).at(-1)?.id ?? '';
      markedUnread.delete(cid);
      return ids;
    }
    case 'mark_contact_unread': {
      const l = list(cid);
      const at = l.findIndex((m) => m.id === a.fromMessageId);
      if (at >= 0) db.readCursor[cid] = l[at - 1]?.id ?? '';
      else markedUnread.add(cid);
      return unreadEntry(cid);
    }
    case 'set_contact_marked_unread':
      if (a.value === true) markedUnread.add(cid);
      else markedUnread.delete(cid);
      return null;
    case 'get_delayed_unseen':
      return {};
    case 'get_pending_sync':
      return { sync: [], deleted: [] };

    case 'send_file_offer': {
      const filename = (a.filePath ?? 'file').split(/[\\/]/).pop() ?? 'file';
      const msg = makeMessage({
        contactId: cid,
        direction: 'sent',
        content: filename,
        sentMs: Date.now(),
        status: 'delivered',
        fileDetails: { filename, sizeBytes: 204_800, autodownloadPath: a.filePath ?? null },
      });
      insert(msg);
      return [msg.id, 204_800, a.filePath ?? ''];
    }
    case 'create_outgoing_pending_path':
      return `/mock/outgoing/pending/${a.filename}`;
    case 'resolve_download_path':
      return `/mock/Downloads/${a.filename}`;
    case 'available_space':
      return 500_000_000_000;
    case 'accept_file_offer':
      simulateDownload(cid, a.offerId ?? '', a.savePath ?? '/mock/Downloads/file');
      return null;
    case 'cancel_file_transfer': {
      const timer = transfers.get(a.offerId ?? '');
      if (timer) clearTimeout(timer);
      transfers.delete(a.offerId ?? '');
      fire('file_transfer_failed', { contactId: cid, transferId: a.offerId, reason: 'cancelled' });
      return null;
    }
    case 'paths_exist':
      return (a.paths ?? []).map(() => true);

    case 'start_call': {
      const callId = randomHex(16);
      calls.set(callId, { connectedAt: null, timers: [] });
      callSteps(callId, [
        ['ringing_out', 50],
        ['connecting', 2000],
        ['connected', 2800],
      ]);
      return callId;
    }
    case 'accept_call':
      callSteps(a.callId ?? '', [
        ['connecting', 100],
        ['connected', 800],
      ]);
      return null;
    case 'decline_call':
      endCall(a.callId ?? '', 'declined');
      return null;
    case 'hangup':
      endCall(a.callId ?? '', 'hangup');
      return null;
    case 'list_audio_devices':
      return {
        inputs: ['Mock Microphone', 'Mock Headset'],
        outputs: ['Mock Speakers', 'Mock Headset'],
        selectedInput: null,
        selectedOutput: null,
      };
    case 'get_call_sample_rate':
      return 48_000;
    case 'list_cameras':
      return [{ id: 'mock-cam', label: 'Mock Camera', facing: null }];
    case 'get_video_quality':
      return 1;
    case 'start_video':
      fire('video_local_state', {
        active: true,
        codec: 'h264',
        width: 1280,
        height: 720,
        cameraId: 'mock-cam',
      });
      return null;
    case 'stop_video':
      fire('video_local_state', {
        active: false,
        codec: null,
        width: 0,
        height: 0,
        cameraId: null,
      });
      return null;

    case 'generate_otp':
      return { otp: 'apple river candle mountain silver ocean tiger violet' };
    case 'check_otp_words':
      return (a.words ?? []).map(() => null);
    case 'fetch_otp':
    case 'import_ltc': {
      const created = makeContact(randomHex(32), 'New Contact', Date.now());
      db.contacts.push(created);
      db.meta[created.userId] = makeMeta(created.userId);
      fire('contact_updated', created);
      return created;
    }
    case 'get_ltc_status':
      return db.ltc;
    case 'create_ltc':
    case 'update_ltc_limits':
    case 'set_ltc_follow_rotations':
    case 'republish_ltc_pointer': {
      const now = Date.now();
      db.ltc ??= {
        payloadId: randomHex(16),
        createdAt: secs(now),
        expiresAt: null,
        maxUses: null,
        uses: 0,
        sizeBytes: 1432,
        followRotations: true,
        pointerState: 'published',
      };
      if (a.maxUses !== undefined) db.ltc.maxUses = a.maxUses;
      if (a.ttlSecs !== undefined) db.ltc.expiresAt = a.ttlSecs ? secs(now) + a.ttlSecs : null;
      if (a.enabled !== undefined) db.ltc.followRotations = a.enabled;
      fire('ltc_updated', db.ltc);
      return cmd === 'set_ltc_follow_rotations' ? null : db.ltc;
    }
    case 'revoke_ltc':
      db.ltc = null;
      fire('ltc_updated', null);
      return null;
    case 'export_ltc':
    case 'export_backup':
      return [75, 85, 82, 83, 65, 76];

    case 'start_nearby':
      return 'Mock session';
    case 'get_nearby_peers':
      return [{ peerId: '12D3KooWNearbyMockPeer', sessionName: 'Emma’s phone', origin: 'mDNS' }];
    case 'accept_nearby':
      addContact('Emma', true);
      return null;

    case 'get_peer_rotation_interval':
      return '30h';
    case 'get_app_lock_config':
      return { enabled: false, method: 'none' };
    case 'verify_app_lock':
      return true;
    case 'get_typing_indicators_enabled':
    case 'get_read_receipts_enabled':
      return true;
    case 'get_relay_config':
      return { maxConnections: 64, maxConnectionsPerIp: 4 };
    case 'get_nodes':
      return { defaults: ['/dns4/relay.kursal.chat/udp/4001/quic-v1'], custom: [] };
    case 'get_network_status':
      return {
        peerCount: 3,
        connectedPeers: [],
        listenAddresses: ['/ip4/127.0.0.1/udp/4001/quic-v1'],
        reachability: 'private',
        dhtServer: false,
        relayActive: false,
        reservations: 1,
        circuits: 0,
        port: 4001,
      };
    case 'get_node_stats':
      return nodeStats();
    case 'start_node_stats':
      if (!statsTimer) statsTimer = setInterval(() => void emit('node_stats', nodeStats()), 1000);
      return null;
    case 'stop_node_stats':
      if (statsTimer) clearInterval(statsTimer);
      statsTimer = null;
      return null;
    case 'get_listening_port':
      return 4001;
    case 'get_nearby_share_enabled':
    case 'get_updater_enabled':
    case 'get_background_mode':
    case 'is_benchmark_running':
      return false;
    case 'list_shared_files':
    case 'list_benchmarks':
    case 'take_pending_shares':
      return [];
    case 'get_auto_accept_config':
      return { mode: 'verified', sizeCapBytes: 25_000_000 };
    case 'get_auto_download_config':
      return { scope: 'all_contacts', limitBytes: 25_000_000 };
    case 'get_storage_usage':
      return {
        logsBytes: 1_200_000,
        dbBytes: 8_400_000,
        filesBytes: 96_000_000,
        avatarsBytes: 320_000,
        perContact: db.contacts.map((c) => ({
          contactId: c.userId,
          dbBytes: 1_000_000,
          filesBytes: 12_000_000,
        })),
      };
    case 'get_update_channel':
      return 'beta';
    case 'get_local_api_config':
      return { enabled: false, hostOnNetwork: false, port: 3030 };
    case 'generate_local_api_token':
      return randomHex(24);
    case 'get_ui_state':
      return uiState.get(a.key ?? '') ?? null;
    case 'set_ui_state':
      uiState.set(a.key ?? '', String(a.value));
      return null;

    case 'log_frontend':
    case 'run_startup_dialogs':
    case 'dialog_respond':
    case 'set_busy_state':
    case 'set_close_explainer_pending':
    case 'set_tray_unread':
    case 'send_typing_indicator':
    case 'send_read_receipts':
    case 'set_delayed_unseen':
    case 'flush_offline':
    case 'video_rx_channel':
    case 'video_tx_channel':
    case 'set_mute':
    case 'set_deafen':
      return null;
  }
  warnOnce(cmd);
  return null;
}

function plugin(cmd: string, a: Args): unknown {
  switch (cmd) {
    case 'plugin:window|is_focused':
      return document.hasFocus();
    case 'plugin:clipboard-manager|write_text':
      return navigator.clipboard.writeText(a.text ?? '').catch(() => null);
    case 'plugin:clipboard-manager|read_text':
      return navigator.clipboard.readText().catch(() => '');
    case 'plugin:opener|open_url':
      window.open(a.url, '_blank', 'noopener');
      return null;
    case 'plugin:notification|is_permission_granted':
      return false;
    case 'plugin:notification|request_permission':
      return 'denied';
  }
  warnOnce(cmd);
  return null;
}

export async function handleCommand(cmd: string, payload?: unknown): Promise<unknown> {
  const args = (payload ?? {}) as Args;
  if (cmd.startsWith('plugin:')) return plugin(cmd, args);
  if (controls.latencyMs > 0 && cmd !== 'log_frontend') await sleep(controls.latencyMs);
  return structuredClone(command(cmd, args));
}

export const peer = {
  contacts: () => db.contacts.map((c) => ({ id: c.userId, name: c.displayName })),
  connection: (contactId: string) => db.connection[contactId],

  message(contactId: string, opts: { sentOffsetMs?: number; viaOffline?: boolean } = {}) {
    const now = Date.now();
    const msg = makeMessage({
      contactId,
      direction: 'received',
      content: pick(SAMPLE_LINES),
      sentMs: now + (opts.sentOffsetMs ?? 0),
      receivedMs: now,
    });
    insert(msg);
    fire('message_received', { ...msg, viaOffline: opts.viaOffline ?? false });
  },
  mailbox: (contactId: string): void =>
    peer.message(contactId, { sentOffsetMs: -3 * HOUR, viaOffline: true }),
  backdated: (contactId: string): void => peer.message(contactId, { sentOffsetMs: -180 * DAY }),
  futureDated: (contactId: string): void => peer.message(contactId, { sentOffsetMs: DAY }),
  burst(contactId: string) {
    for (let i = 0; i < 20; i++) setTimeout(() => peer.message(contactId), i * 120);
  },
  typing: (contactId: string) => fire('typing_indicator', { contactId, replyTo: null }),

  readAll(contactId: string) {
    const unread = list(contactId).filter(
      (m) => m.direction === 'sent' && m.status !== 'read' && m.status !== 'failed'
    );
    unread.forEach((m) => (m.status = 'read'));
    fire('messages_read', { contactId, messageIds: unread.map((m) => m.id) });
  },
  react(contactId: string) {
    const msg = lastWhere(contactId, (m) => !m.callDetails);
    if (!msg) return;
    const emoji = pick(['👍', '❤️', '😂', '🔥', '😮']);
    msg.reactions = [...(msg.reactions ?? []), { emoji, userId: contactId }];
    fire('reaction_added', { contactId, messageId: msg.id, emoji, userId: contactId });
  },
  editLast(contactId: string) {
    const msg = lastWhere(contactId, (m) => m.direction === 'received' && !m.callDetails);
    if (!msg) return;
    msg.content = pick(SAMPLE_LINES);
    msg.edited = true;
    fire('message_edited', { contactId, messageId: msg.id, newContent: msg.content });
  },
  deleteLast(contactId: string) {
    const msg = lastWhere(contactId, (m) => m.direction === 'received');
    if (!msg) return;
    remove(contactId, msg.id);
    fire('message_deleted', { contactId, messageId: msg.id });
  },
  fileOffer(contactId: string) {
    const filename = pick(['holiday.jpg', 'notes.txt', 'report.pdf', 'voice-memo.m4a']);
    const sizeBytes = 200_000 + Math.floor(Math.random() * 8_000_000);
    const msg = makeMessage({
      contactId,
      direction: 'received',
      content: filename,
      sentMs: Date.now(),
      fileDetails: { filename, sizeBytes, autodownloadPath: null },
    });
    insert(msg);
    fire('file_offered', { offerId: msg.id, contactId, filename, sizeBytes, autodownload: null });
  },
  gap: (contactId: string) => fire('offline_gap_skipped', { contactId, counter: 3 }),

  call(contactId: string) {
    const callId = randomHex(16);
    calls.set(callId, { connectedAt: null, timers: [] });
    fire('call_incoming', { callId, contactId, sampleRate: 48_000 });
  },
  hangupAll(reason: string) {
    [...calls.keys()].forEach((callId) => endCall(callId, reason));
  },

  setConnection(contactId: string, status: ConnectionStatus) {
    db.connection[contactId] = status;
    fire('connection_changed', { contactId, status });
  },
  toggleTerminated(contactId: string) {
    const meta = db.meta[contactId];
    if (!meta) return;
    meta.terminated = !meta.terminated;
    fire('contact_terminated', { contactId, terminated: meta.terminated });
  },
  rename(contactId: string) {
    const c = contact(contactId);
    if (!c) return;
    c.displayName = pick(['Arlo', 'Kubik', 'Koschi', 'Art', 'Nite']);
    c.profileName = c.displayName;
    fire('contact_updated', c);
  },
  addContact: () => addContact(pick(['Erik', 'Armand']), false),
};

export const app = {
  network: (online: boolean) => fire('network_online', { online, peerCount: online ? 3 : 0 }),
  offlineSync: (active: boolean) => fire('offline_sync', { active }),
  incomingError: () =>
    fire('backend_signal', { signal: 'handle_incoming_error', payload: 'mock failure' }),
  nearbyRequest: () =>
    fire('nearby_request', { peerId: '12D3KooWNearbyMockPeer', sessionName: "Kubik's phone" }),
  updateDownload() {
    const total = 42_000_000;
    for (let i = 1; i <= 20; i++) {
      fire(
        'update_download_progress',
        { downloaded: (total / 20) * i, contentLength: total },
        i * 150
      );
    }
    fire('update_download_finished', null, 21 * 150);
  },
};
