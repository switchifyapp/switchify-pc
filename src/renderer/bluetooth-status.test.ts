import { describe, expect, it } from 'vitest';
import type { BluetoothStatus } from '../shared/bluetooth-status';
import {
  formatBluetoothDiagnosticEvent,
  formatBluetoothDisconnectReason,
  formatBluetoothStatus
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

  it('formats safe diagnostic events', () => {
    expect(formatBluetoothDiagnosticEvent('write_received')).toBe('Message received.');
    expect(formatBluetoothDiagnosticEvent('unsubscribed')).toBe('Device unsubscribed.');
    expect(formatBluetoothDiagnosticEvent('unsubscribe_grace_started')).toBe('Waiting for Bluetooth reconnect.');
    expect(formatBluetoothDiagnosticEvent('unsubscribe_grace_timed_out')).toBe('Bluetooth reconnect timed out.');
    expect(formatBluetoothDiagnosticEvent(null)).toBe('Not recorded.');
  });

  it('formats safe disconnect reasons', () => {
    expect(formatBluetoothDisconnectReason('notification_unsubscribed')).toBe('Bluetooth connection lost.');
    expect(formatBluetoothDisconnectReason('pc_requested')).toBe('Disconnected from this PC.');
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
    status: 'ready',
    reason: null,
    connectedClientCount: 0,
    lastError: null,
    lastEvent: null,
    lastEventAt: null,
    recentEvents: [],
    lastDisconnectReason: null,
    lastDisconnectAt: null,
    ...overrides
  };
}
