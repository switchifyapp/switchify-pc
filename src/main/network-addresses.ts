import { networkInterfaces, type NetworkInterfaceInfo } from 'node:os';

export type NetworkAddressCandidate = {
  address: string;
  family: 'IPv4' | 'IPv6';
  websocketUrl: string;
  isLoopback: boolean;
};

export function getConnectionCandidates(port: number): NetworkAddressCandidate[] {
  return toConnectionCandidates(Object.values(networkInterfaces()).flat(), port);
}

export function toConnectionCandidates(
  interfaces: Array<NetworkInterfaceInfo | undefined>,
  port: number
): NetworkAddressCandidate[] {
  const items = interfaces.filter((item): item is NetworkInterfaceInfo => Boolean(item));
  const globalIpv6 = uniqueAddresses(
    items
      .filter((item) => item.family === 'IPv6' && !item.internal)
      .map((item) => item.address)
      .filter(isGlobalIPv6Address)
  );
  const privateIpv4 = uniqueAddresses(
    items
      .filter((item) => item.family === 'IPv4' && !item.internal)
      .map((item) => item.address)
      .filter(isPrivateIPv4Address)
  );
  const ulaIpv6 = uniqueAddresses(
    items
      .filter((item) => item.family === 'IPv6' && !item.internal)
      .map((item) => item.address)
      .filter(isUniqueLocalIPv6Address)
  );
  const otherIpv4 = uniqueAddresses(
    items
      .filter((item) => item.family === 'IPv4' && !item.internal)
      .map((item) => item.address)
      .filter((address) => isUsefulIPv4Address(address) && !isPrivateIPv4Address(address))
  );
  const addresses = [
    ...globalIpv6.map((address) => ({ address, family: 'IPv6' as const, isLoopback: false })),
    ...privateIpv4.map((address) => ({ address, family: 'IPv4' as const, isLoopback: false })),
    ...ulaIpv6.map((address) => ({ address, family: 'IPv6' as const, isLoopback: false })),
    ...otherIpv4.map((address) => ({ address, family: 'IPv4' as const, isLoopback: false })),
    { address: '127.0.0.1', family: 'IPv4' as const, isLoopback: true },
    { address: '::1', family: 'IPv6' as const, isLoopback: true }
  ];

  return addresses.map((candidate) => ({
    ...candidate,
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

function uniqueAddresses(addresses: string[]): string[] {
  return [...new Set(addresses)].sort();
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
