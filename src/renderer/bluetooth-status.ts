import type { BluetoothDiagnosticEvent, BluetoothDisconnectReason, BluetoothStatus } from '../shared/bluetooth-status';

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

export function formatBluetoothDiagnosticEvent(event: BluetoothDiagnosticEvent): string {
  if (event === 'advertising_started') return 'Advertising started.';
  if (event === 'subscribed') return 'Device subscribed.';
  if (event === 'unsubscribe_grace_started') return 'Waiting for Bluetooth reconnect.';
  if (event === 'unsubscribe_grace_cancelled') return 'Bluetooth reconnect resumed.';
  if (event === 'write_received') return 'Message received.';
  return 'Not recorded.';
}

export function formatBluetoothDisconnectReason(reason: BluetoothDisconnectReason): string {
  if (reason === 'notification_unsubscribed') return 'Bluetooth connection lost.';
  if (reason === 'pc_requested') return 'Disconnected from this PC.';
  if (reason === 'helper_stopped') return 'Bluetooth helper stopped.';
  if (reason === 'helper_error') return 'Bluetooth helper error.';
  return 'Not recorded.';
}

