import { describe, expect, it } from 'vitest';
import type { NetworkInterfaceInfo } from 'node:os';
import { formatWebSocketUrl, toConnectionCandidates } from './network-addresses';

describe('toConnectionCandidates', () => {
  it('returns global IPv6 and LAN IPv4 websocket URLs before loopback', () => {
    const candidates = toConnectionCandidates(
      [
        ipv6('2001:bb6:a61:3700:574c:69d2:25ce:505', false),
        ipv6('fd12::1', false),
        ipv4('192.168.1.20', false),
        ipv4('10.0.0.5', false),
        ipv4('127.0.0.1', true),
        ipv6('fe80::1', false)
      ],
      7347
    );

    expect(candidates).toEqual([
      {
        address: '2001:bb6:a61:3700:574c:69d2:25ce:505',
        family: 'IPv6',
        websocketUrl: 'ws://[2001:bb6:a61:3700:574c:69d2:25ce:505]:7347',
        isLoopback: false
      },
      { address: '10.0.0.5', family: 'IPv4', websocketUrl: 'ws://10.0.0.5:7347', isLoopback: false },
      { address: '192.168.1.20', family: 'IPv4', websocketUrl: 'ws://192.168.1.20:7347', isLoopback: false },
      { address: 'fd12::1', family: 'IPv6', websocketUrl: 'ws://[fd12::1]:7347', isLoopback: false },
      { address: '127.0.0.1', family: 'IPv4', websocketUrl: 'ws://127.0.0.1:7347', isLoopback: true },
      { address: '::1', family: 'IPv6', websocketUrl: 'ws://[::1]:7347', isLoopback: true }
    ]);
  });

  it('deduplicates LAN addresses and ignores link-local addresses', () => {
    const candidates = toConnectionCandidates(
      [
        ipv4('192.168.1.20', false),
        ipv4('192.168.1.20', false),
        ipv4('169.254.10.1', false),
        ipv6('fe80::abcd', false),
        ipv6('::', false)
      ],
      7347
    );

    expect(candidates).toEqual([
      { address: '192.168.1.20', family: 'IPv4', websocketUrl: 'ws://192.168.1.20:7347', isLoopback: false },
      { address: '127.0.0.1', family: 'IPv4', websocketUrl: 'ws://127.0.0.1:7347', isLoopback: true },
      { address: '::1', family: 'IPv6', websocketUrl: 'ws://[::1]:7347', isLoopback: true }
    ]);
  });

  it('falls back to loopback when no LAN address is available', () => {
    expect(toConnectionCandidates([ipv4('127.0.0.1', true)], 7347)).toEqual([
      { address: '127.0.0.1', family: 'IPv4', websocketUrl: 'ws://127.0.0.1:7347', isLoopback: true },
      { address: '::1', family: 'IPv6', websocketUrl: 'ws://[::1]:7347', isLoopback: true }
    ]);
  });

  it('formats websocket URLs for IPv4, IPv6, and already bracketed IPv6 addresses', () => {
    expect(formatWebSocketUrl('192.168.1.180', 7347)).toBe('ws://192.168.1.180:7347');
    expect(formatWebSocketUrl('2001:bb6:a61:3700:574c:69d2:25ce:505', 7347)).toBe(
      'ws://[2001:bb6:a61:3700:574c:69d2:25ce:505]:7347'
    );
    expect(formatWebSocketUrl('[2001:bb6:a61:3700:574c:69d2:25ce:505]', 7347)).toBe(
      'ws://[2001:bb6:a61:3700:574c:69d2:25ce:505]:7347'
    );
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

function ipv6(address: string, internal: boolean): NetworkInterfaceInfo {
  return {
    address,
    netmask: 'ffff:ffff:ffff:ffff::',
    family: 'IPv6',
    mac: '00:00:00:00:00:00',
    internal,
    cidr: `${address}/64`,
    scopeid: 0
  };
}
