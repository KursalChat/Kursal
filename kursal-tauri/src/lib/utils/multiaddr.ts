export type ParsedAddress = {
  transport: string | null;
  host: string | null;
  port: string | null;
  peerId: string | null;
  relay: boolean;
  raw: string;
};

const HOST_PROTOS = new Set(['ip4', 'ip6', 'dns', 'dns4', 'dns6', 'dnsaddr']);
const VALUED_PROTOS = new Set([
  'ip4',
  'ip6',
  'dns',
  'dns4',
  'dns6',
  'dnsaddr',
  'tcp',
  'udp',
  'p2p',
  'sni',
  'certhash',
]);

const TRANSPORT_LABELS: Record<string, string> = {
  quic: 'QUIC',
  'quic-v1': 'QUIC',
  ws: 'WS',
  wss: 'WSS',
  webrtc: 'WebRTC',
  'webrtc-direct': 'WebRTC',
  webtransport: 'WebTransport',
};

export function parseMultiaddr(raw: string): ParsedAddress {
  const result: ParsedAddress = {
    transport: null,
    host: null,
    port: null,
    peerId: null,
    relay: false,
    raw,
  };

  const parts = raw.split('/').filter((p) => p.length > 0);
  if (parts.length === 0) return result;

  let sawUdp = false;
  let i = 0;
  while (i < parts.length) {
    const proto = parts[i];
    if (VALUED_PROTOS.has(proto)) {
      const value = parts[i + 1] ?? '';
      i += 2;
      if (HOST_PROTOS.has(proto)) {
        result.host = value;
      } else if (proto === 'tcp' || proto === 'udp') {
        result.port = value;
        if (proto === 'udp') sawUdp = true;
        if (!result.transport) result.transport = proto === 'tcp' ? 'TCP' : null;
      } else if (proto === 'p2p') {
        result.peerId = value;
      }
    } else {
      if (proto === 'p2p-circuit') result.relay = true;
      else if (TRANSPORT_LABELS[proto]) result.transport = TRANSPORT_LABELS[proto];
      i += 1;
    }
  }

  if (!result.transport && sawUdp) result.transport = 'QUIC';

  return result;
}

const TRANSPORT_ORDER = ['QUIC', 'TCP'];

function addressRank(raw: string): number {
  const info = parseMultiaddr(raw);
  const transport = TRANSPORT_ORDER.indexOf(info.transport ?? '');
  return (info.relay ? 100 : 0) + (transport === -1 ? TRANSPORT_ORDER.length : transport);
}

export function sortAddresses(addresses: string[]): string[] {
  return [...addresses].sort((a, b) => addressRank(a) - addressRank(b) || a.localeCompare(b));
}

export function shortPeerId(id: string | null): string | null {
  if (!id) return null;
  if (id.length <= 12) return id;
  return `${id.slice(0, 4)}…${id.slice(-8)}`;
}
