import type { BluetoothStatus } from '../shared/bluetooth-status';

export function formatBluetoothStatus(status: BluetoothStatus | null | undefined): string {
  if (!status) return 'Bluetooth status unknown.';

  if (status.status === 'ready') return 'Bluetooth ready.';
  if (status.status === 'connected') {
    return status.connectedClientCount === 1 ? 'Bluetooth device connected.' : 'Bluetooth devices connected.';
  }
  if (status.status === 'starting') return 'Starting Bluetooth...';
  if (status.status === 'stopped') return 'Bluetooth stopped.';
  if (status.status === 'disabled') return 'Bluetooth disabled.';
  if (status.status === 'unavailable') return toUnavailableText(status.reason);
  return 'Bluetooth needs attention.';
}

function toUnavailableText(reason: BluetoothStatus['reason']): string {
  if (reason === 'adapter_off') return 'Bluetooth is off.';
  if (reason === 'permission_denied') return 'Bluetooth permission denied.';
  if (reason === 'unsupported') return 'Bluetooth unavailable.';
  return 'Bluetooth unavailable.';
}

