import type { BluetoothDiagnosticEvent, BluetoothDisconnectReason, BluetoothStatus } from '../shared/bluetooth-status';

export function formatBluetoothStatus(status: BluetoothStatus | null | undefined): string {
  if (!status) return 'Bluetooth status unknown.';

  if (!status.system.adapterPresent) return 'Bluetooth adapter not found.';
  if (status.system.radioState === 'off') return 'Bluetooth radio off.';
  if (status.system.radioState === 'disabled') return 'Bluetooth radio disabled.';
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

export function formatBluetoothSystemRadioState(status: BluetoothStatus | null | undefined): string {
  if (!status) return 'Bluetooth radio state unknown.';
  if (!status.system.adapterPresent) return 'Bluetooth adapter not found.';
  if (status.system.radioState === 'on') return 'Bluetooth radio on.';
  if (status.system.radioState === 'off') return 'Bluetooth radio off.';
  if (status.system.radioState === 'disabled') return 'Bluetooth radio disabled.';
  return 'Bluetooth radio state unknown.';
}

export function formatBluetoothSystemCapabilities(status: BluetoothStatus | null | undefined): string {
  const system = status?.system;
  if (!system || system.isLowEnergySupported === null || system.isPeripheralRoleSupported === null) {
    return 'Bluetooth capabilities unknown.';
  }

  if (system.isLowEnergySupported && system.isPeripheralRoleSupported) {
    return 'Bluetooth LE peripheral supported.';
  }

  return 'Bluetooth LE peripheral not supported.';
}

function toUnavailableText(reason: BluetoothStatus['reason']): string {
  if (reason === 'adapter_off') return 'Bluetooth is off.';
  if (reason === 'permission_denied') return 'Bluetooth permission denied.';
  if (reason === 'unsupported') return 'Bluetooth unavailable.';
  return 'Bluetooth unavailable.';
}

export function formatBluetoothDiagnosticEvent(event: BluetoothDiagnosticEvent): string {
  if (event === 'advertising_started') return 'Advertising started.';
  if (event === 'advertising_restarted') return 'Advertising restarted.';
  if (event === 'system_radio_on') return 'Bluetooth turned on.';
  if (event === 'system_radio_off') return 'Bluetooth turned off.';
  if (event === 'subscribed') return 'Device subscribed.';
  if (event === 'unsubscribed') return 'Device unsubscribed.';
  if (event === 'unsubscribe_grace_started') return 'Waiting for Bluetooth reconnect.';
  if (event === 'unsubscribe_grace_cancelled') return 'Bluetooth reconnect resumed.';
  if (event === 'unsubscribe_grace_timed_out') return 'Bluetooth reconnect timed out.';
  if (event === 'write_received') return 'Message received.';
  return 'Not recorded.';
}

export function formatBluetoothDisconnectReason(reason: BluetoothDisconnectReason): string {
  if (reason === 'notification_unsubscribed') return 'Bluetooth connection lost.';
  if (reason === 'client_requested') return 'Android device disconnected.';
  if (reason === 'pc_requested') return 'Disconnected from this PC.';
  if (reason === 'helper_stopped') return 'Bluetooth helper stopped.';
  if (reason === 'helper_error') return 'Bluetooth helper error.';
  if (reason === 'adapter_off') return 'Bluetooth was turned off.';
  return 'Not recorded.';
}

