import { networkInterfaces, type NetworkInterfaceInfo } from 'node:os';

export type NetworkAddressCandidate = {
  address: string;
  family: 'IPv4' | 'IPv6';
  websocketUrl: string;
  isLoopback: boolean;
  interfaceName?: string;
};

type NetworkInterfaceCandidate = {
  interfaceName: string;
  info: NetworkInterfaceInfo;
};

type RankedNetworkAddressCandidate = NetworkAddressCandidate & {
  rank: number;
};

export function getConnectionCandidates(port: number): NetworkAddressCandidate[] {
  const interfaces = Object.entries(networkInterfaces()).flatMap(([interfaceName, addresses]) =>
    (addresses ?? []).map((info) => ({ interfaceName, info }))
  );

  return toConnectionCandidates(interfaces, port);
}

export function toConnectionCandidates(
  interfaces: Array<NetworkInterfaceCandidate | NetworkInterfaceInfo | undefined>,
  port: number
): NetworkAddressCandidate[] {
  const rankedCandidates = normalizeInterfaces(interfaces)
    .filter(({ info }) => !info.internal)
    .map(toRankedCandidate)
    .filter((candidate): candidate is RankedNetworkAddressCandidate => Boolean(candidate));
  const addresses = [
    ...dedupeByBestRank(rankedCandidates).sort(compareRankedCandidates),
    { address: '127.0.0.1', family: 'IPv4' as const, isLoopback: true, interfaceName: undefined, rank: 7 },
    { address: '::1', family: 'IPv6' as const, isLoopback: true, interfaceName: undefined, rank: 7 }
  ];

  return addresses.map(({ rank: _rank, interfaceName, ...candidate }) => ({
    ...candidate,
    ...(interfaceName ? { interfaceName } : {}),
    websocketUrl: formatWebSocketUrl(candidate.address, port)
  }));
}

export function formatWebSocketUrl(address: string, port: number): string {
  const normalizedAddress = address.trim();
  const host =
    normalizedAddress.includes(':') && !normalizedAddress.startsWith('[')
      ? `[${normalizedAddress}]`
      : normalizedAddress;

  return `ws://${host}:${port}`;
}

function normalizeInterfaces(
  interfaces: Array<NetworkInterfaceCandidate | NetworkInterfaceInfo | undefined>
): NetworkInterfaceCandidate[] {
  return interfaces
    .filter((item): item is NetworkInterfaceCandidate | NetworkInterfaceInfo => Boolean(item))
    .map((item) =>
      'info' in item
        ? item
        : {
            interfaceName: '',
            info: item
          }
    );
}

function toRankedCandidate(candidate: NetworkInterfaceCandidate): RankedNetworkAddressCandidate | null {
  const { interfaceName, info } = candidate;
  if (info.family === 'IPv4' && isUsefulIPv4Address(info.address)) {
    const rank = rankIPv4Address(interfaceName, info.address);
    if (rank === null) return null;
    return {
      address: info.address,
      family: 'IPv4',
      websocketUrl: '',
      isLoopback: false,
      interfaceName,
      rank
    };
  }

  if (info.family === 'IPv6' && isUsefulIPv6Address(info.address)) {
    const rank = rankIPv6Address(interfaceName, info.address);
    if (rank === null) return null;
    return {
      address: info.address,
      family: 'IPv6',
      websocketUrl: '',
      isLoopback: false,
      interfaceName,
      rank
    };
  }

  return null;
}

function rankIPv4Address(interfaceName: string, address: string): number | null {
  if (isVirtualAdapter(interfaceName)) return 6;
  if (isVpnAdapter(interfaceName)) return 5;
  if (isPrivateIPv4Address(address) && isPhysicalLanAdapter(interfaceName)) return 1;
  if (isPrivateIPv4Address(address)) return 2;
  return 4;
}

function rankIPv6Address(interfaceName: string, address: string): number | null {
  if (isVirtualAdapter(interfaceName)) return 6;
  if (isVpnAdapter(interfaceName)) return 5;
  if (isUniqueLocalIPv6Address(address)) return 3;
  if (isGlobalIPv6Address(address)) return 4;
  return null;
}

function dedupeByBestRank(candidates: RankedNetworkAddressCandidate[]): RankedNetworkAddressCandidate[] {
  const byAddress = new Map<string, RankedNetworkAddressCandidate>();

  for (const candidate of candidates) {
    const existing = byAddress.get(candidate.address);
    if (!existing || compareRankedCandidates(candidate, existing) < 0) {
      byAddress.set(candidate.address, candidate);
    }
  }

  return [...byAddress.values()];
}

function compareRankedCandidates(left: RankedNetworkAddressCandidate, right: RankedNetworkAddressCandidate): number {
  return (
    left.rank - right.rank ||
    familyOrder(left.family) - familyOrder(right.family) ||
    (left.interfaceName ?? '').localeCompare(right.interfaceName ?? '') ||
    left.address.localeCompare(right.address)
  );
}

function familyOrder(family: 'IPv4' | 'IPv6'): number {
  return family === 'IPv4' ? 0 : 1;
}

function isUsefulIPv4Address(address: string): boolean {
  return !address.startsWith('169.254.') && address !== '0.0.0.0';
}

function isPrivateIPv4Address(address: string): boolean {
  return (
    address.startsWith('10.') ||
    address.startsWith('192.168.') ||
    is172PrivateIPv4Address(address)
  );
}

function is172PrivateIPv4Address(address: string): boolean {
  const parts = address.split('.');
  if (parts.length < 2 || parts[0] !== '172') return false;
  const second = Number(parts[1]);
  return Number.isInteger(second) && second >= 16 && second <= 31;
}

function isGlobalIPv6Address(address: string): boolean {
  return isUsefulIPv6Address(address) && !isUniqueLocalIPv6Address(address);
}

function isUniqueLocalIPv6Address(address: string): boolean {
  const normalizedAddress = address.toLowerCase();
  return normalizedAddress.startsWith('fc') || normalizedAddress.startsWith('fd');
}

function isUsefulIPv6Address(address: string): boolean {
  const normalizedAddress = address.toLowerCase();
  return (
    normalizedAddress !== '::' &&
    !normalizedAddress.startsWith('fe80:') &&
    !normalizedAddress.startsWith('fe80::')
  );
}

function isPhysicalLanAdapter(interfaceName: string): boolean {
  if (isVirtualAdapter(interfaceName) || isVpnAdapter(interfaceName)) return false;

  return /(^|\b)(wi-?fi|wlan|ethernet|local area connection)(\b|$)/i.test(interfaceName);
}

function isVpnAdapter(interfaceName: string): boolean {
  return /(tailscale|zerotier|wireguard|openvpn|vpn)/i.test(interfaceName);
}

function isVirtualAdapter(interfaceName: string): boolean {
  return /(vethernet|wsl|hyper-v|virtualbox|vmware|loopback|docker)/i.test(interfaceName);
}
