import { describe, expect, it } from 'vitest';
import { DEFAULT_BLUETOOTH_STATUS, type BluetoothStatus } from '../shared/bluetooth-status';
import {
  formatBluetoothDiagnosticEvent,
  formatBluetoothDisconnectReason,
  formatBluetoothStatus,
  formatBluetoothSystemCapabilities,
  formatBluetoothSystemRadioState
} from './bluetooth-status';

describe('formatBluetoothStatus', () => {
  it('formats ready status', () => {
    expect(formatBluetoothStatus(bluetoothStatus({ status: 'ready' }))).toBe('Bluetooth ready.');
  });

  it('formats connected status', () => {
    expect(formatBluetoothStatus(bluetoothStatus({ status: 'connected', connectedClientCount: 1 }))).toBe('Bluetooth device connected.');
  });

  it('formats adapter off without low-level details', () => {
    expect(formatBluetoothStatus(bluetoothStatus({ status: 'unavailable', reason: 'adapter_off' }))).toBe('Bluetooth is off.');
  });

  it('formats live system radio state', () => {
    expect(formatBluetoothSystemRadioState(bluetoothStatus())).toBe('Bluetooth radio on.');
    expect(
      formatBluetoothSystemRadioState(
        bluetoothStatus({
          system: {
            adapterPresent: true,
            radioState: 'off',
            isLowEnergySupported: true,
            isPeripheralRoleSupported: true,
            lastCheckedAt: 1,
            lastChangedAt: 1
          }
        })
      )
    ).toBe('Bluetooth radio off.');
    expect(
      formatBluetoothSystemRadioState(
        bluetoothStatus({
          system: {
            adapterPresent: true,
            radioState: 'disabled',
            isLowEnergySupported: true,
            isPeripheralRoleSupported: true,
            lastCheckedAt: 1,
            lastChangedAt: 1
          }
        })
      )
    ).toBe('Bluetooth radio disabled.');
    expect(formatBluetoothSystemRadioState(bluetoothStatus({ system: DEFAULT_BLUETOOTH_STATUS.system }))).toBe(
      'Bluetooth adapter not found.'
    );
  });

  it('formats live Bluetooth capabilities', () => {
    expect(formatBluetoothSystemCapabilities(bluetoothStatus())).toBe('Bluetooth LE peripheral supported.');
    expect(
      formatBluetoothSystemCapabilities(
        bluetoothStatus({
          system: {
            adapterPresent: true,
            radioState: 'on',
            isLowEnergySupported: true,
            isPeripheralRoleSupported: false,
            lastCheckedAt: 1,
            lastChangedAt: 1
          }
        })
      )
    ).toBe('Bluetooth LE peripheral not supported.');
    expect(formatBluetoothSystemCapabilities(bluetoothStatus({ system: DEFAULT_BLUETOOTH_STATUS.system }))).toBe(
      'Bluetooth capabilities unknown.'
    );
  });

  it('formats safe diagnostic events', () => {
    expect(formatBluetoothDiagnosticEvent('advertising_restarted')).toBe('Advertising restarted.');
    expect(formatBluetoothDiagnosticEvent('system_radio_on')).toBe('Bluetooth turned on.');
    expect(formatBluetoothDiagnosticEvent('system_radio_off')).toBe('Bluetooth turned off.');
    expect(formatBluetoothDiagnosticEvent('write_received')).toBe('Message received.');
    expect(formatBluetoothDiagnosticEvent('unsubscribed')).toBe('Device unsubscribed.');
    expect(formatBluetoothDiagnosticEvent('unsubscribe_grace_started')).toBe('Waiting for Bluetooth reconnect.');
    expect(formatBluetoothDiagnosticEvent('unsubscribe_grace_timed_out')).toBe('Bluetooth reconnect timed out.');
    expect(formatBluetoothDiagnosticEvent(null)).toBe('Not recorded.');
  });

  it('formats safe disconnect reasons', () => {
    expect(formatBluetoothDisconnectReason('notification_unsubscribed')).toBe('Bluetooth connection lost.');
    expect(formatBluetoothDisconnectReason('client_requested')).toBe('Android device disconnected.');
    expect(formatBluetoothDisconnectReason('pc_requested')).toBe('Disconnected from this PC.');
    expect(formatBluetoothDisconnectReason('adapter_off')).toBe('Bluetooth was turned off.');
    expect(formatBluetoothDisconnectReason(null)).toBe('Not recorded.');
  });

  it('does not include raw frame or payload details in diagnostic copy', () => {
    const text = [
      formatBluetoothDiagnosticEvent('write_received'),
      formatBluetoothDisconnectReason('notification_unsubscribed')
    ].join(' ');

    expect(text).not.toMatch(/payload|frame|token|auth|secret|typed/i);
  });
});

function bluetoothStatus(overrides: Partial<BluetoothStatus> = {}): BluetoothStatus {
  return {
    ...DEFAULT_BLUETOOTH_STATUS,
    status: 'ready',
    system: {
      ...DEFAULT_BLUETOOTH_STATUS.system,
      adapterPresent: true,
      radioState: 'on',
      isLowEnergySupported: true,
      isPeripheralRoleSupported: true
    },
    ...overrides
  };
}
