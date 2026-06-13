import { describe, expect, it } from 'vitest';
import type { NetworkInterfaceInfo } from 'node:os';
import { formatWebSocketUrl, toConnectionCandidates } from './network-addresses';

describe('toConnectionCandidates', () => {
  it('prefers Wi-Fi LAN address over Tailscale and WSL virtual adapters', () => {
    const candidates = toConnectionCandidates(
      [
        named('Tailscale', ipv4('100.66.217.56', false)),
        named('vEthernet (WSL (Hyper-V firewall))', ipv4('172.22.192.1', false)),
        named('Wi-Fi', ipv4('192.168.1.180', false))
      ],
      7347
    );

    expect(candidates.map((candidate) => candidate.websocketUrl)).toEqual([
      'ws://192.168.1.180:7347',
      'ws://100.66.217.56:7347',
      'ws://172.22.192.1:7347',
      'ws://127.0.0.1:7347',
      'ws://[::1]:7347'
    ]);
  });

  it('prefers Ethernet private IPv4 before global IPv6', () => {
    const candidates = toConnectionCandidates(
      [
        named('Ethernet', ipv6('2001:bb6:a61:3700:574c:69d2:25ce:505', false)),
        named('Ethernet', ipv4('10.0.0.5', false))
      ],
      7347
    );

    expect(candidates.map((candidate) => candidate.websocketUrl).slice(0, 2)).toEqual([
      'ws://10.0.0.5:7347',
      'ws://[2001:bb6:a61:3700:574c:69d2:25ce:505]:7347'
    ]);
  });

  it('keeps Tailscale as a fallback after Wi-Fi or Ethernet', () => {
    const candidates = toConnectionCandidates(
      [named('Tailscale', ipv4('100.66.217.56', false)), named('Ethernet', ipv4('192.168.1.20', false))],
      7347
    );

    expect(candidates.map((candidate) => candidate.websocketUrl).slice(0, 2)).toEqual([
      'ws://192.168.1.20:7347',
      'ws://100.66.217.56:7347'
    ]);
  });

  it('keeps WSL and Hyper-V addresses after real LAN and VPN candidates', () => {
    const candidates = toConnectionCandidates(
      [
        named('vEthernet (WSL (Hyper-V firewall))', ipv4('172.22.192.1', false)),
        named('Tailscale', ipv4('100.66.217.56', false)),
        named('Wi-Fi', ipv4('192.168.1.180', false))
      ],
      7347
    );

    expect(candidates.map((candidate) => candidate.websocketUrl).slice(0, 3)).toEqual([
      'ws://192.168.1.180:7347',
      'ws://100.66.217.56:7347',
      'ws://172.22.192.1:7347'
    ]);
  });

  it('keeps loopback addresses last', () => {
    const candidates = toConnectionCandidates(
      [named('Wi-Fi', ipv4('192.168.1.180', false)), ipv4('127.0.0.1', true), ipv6('::1', true)],
      7347
    );

    expect(candidates.map((candidate) => candidate.websocketUrl).slice(-2)).toEqual([
      'ws://127.0.0.1:7347',
      'ws://[::1]:7347'
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

function named(interfaceName: string, info: NetworkInterfaceInfo): { interfaceName: string; info: NetworkInterfaceInfo } {
  return { interfaceName, info };
}

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
