import { networkInterfaces, type NetworkInterfaceInfo } from 'node:os';

export type NetworkAddressCandidate = {
  address: string;
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
  const lanAddresses = interfaces
    .filter((item): item is NetworkInterfaceInfo => Boolean(item))
    .filter((item) => item.family === 'IPv4' && !item.internal)
    .map((item) => item.address)
    .filter(isUsefulIPv4Address);
  const uniqueLanAddresses = [...new Set(lanAddresses)].sort();
  const addresses = [...uniqueLanAddresses, '127.0.0.1'];

  return addresses.map((address) => ({
    address,
    websocketUrl: `ws://${address}:${port}`,
    isLoopback: address === '127.0.0.1'
  }));
}

function isUsefulIPv4Address(address: string): boolean {
  return !address.startsWith('169.254.') && address !== '0.0.0.0';
}
