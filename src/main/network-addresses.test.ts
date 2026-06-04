import { describe, expect, it } from 'vitest';
import type { NetworkInterfaceInfo } from 'node:os';
import { toConnectionCandidates } from './network-addresses';

describe('toConnectionCandidates', () => {
  it('returns LAN websocket URLs before loopback', () => {
    const candidates = toConnectionCandidates(
      [
        ipv4('192.168.1.20', false),
        ipv4('10.0.0.5', false),
        ipv4('127.0.0.1', true),
        { ...ipv4('fe80::1', false), family: 'IPv6', scopeid: 1 }
      ],
      7347
    );

    expect(candidates).toEqual([
      { address: '10.0.0.5', websocketUrl: 'ws://10.0.0.5:7347', isLoopback: false },
      { address: '192.168.1.20', websocketUrl: 'ws://192.168.1.20:7347', isLoopback: false },
      { address: '127.0.0.1', websocketUrl: 'ws://127.0.0.1:7347', isLoopback: true }
    ]);
  });

  it('deduplicates LAN addresses and ignores link-local addresses', () => {
    const candidates = toConnectionCandidates(
      [ipv4('192.168.1.20', false), ipv4('192.168.1.20', false), ipv4('169.254.10.1', false)],
      7347
    );

    expect(candidates).toEqual([
      { address: '192.168.1.20', websocketUrl: 'ws://192.168.1.20:7347', isLoopback: false },
      { address: '127.0.0.1', websocketUrl: 'ws://127.0.0.1:7347', isLoopback: true }
    ]);
  });

  it('falls back to loopback when no LAN address is available', () => {
    expect(toConnectionCandidates([ipv4('127.0.0.1', true)], 7347)).toEqual([
      { address: '127.0.0.1', websocketUrl: 'ws://127.0.0.1:7347', isLoopback: true }
    ]);
  });
});

function ipv4(address: string, internal: boolean): NetworkInterfaceInfo {
  return {
    address,
    netmask: '255.255.255.0',
    family: 'IPv4',
    mac: '00:00:00:00:00:00',
    internal,
    cidr: `${address}/24`
  };
}
