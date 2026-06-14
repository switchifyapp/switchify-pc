import { describe, expect, it } from 'vitest';
import { formatBluetoothStatus } from './bluetooth-status';

describe('formatBluetoothStatus', () => {
  it('formats ready status', () => {
    expect(formatBluetoothStatus({ status: 'ready', reason: null, connectedClientCount: 0, lastError: null })).toBe(
      'Bluetooth ready.'
    );
  });

  it('formats connected status', () => {
    expect(formatBluetoothStatus({ status: 'connected', reason: null, connectedClientCount: 1, lastError: null })).toBe(
      'Bluetooth device connected.'
    );
  });

  it('formats adapter off without low-level details', () => {
    expect(formatBluetoothStatus({ status: 'unavailable', reason: 'adapter_off', connectedClientCount: 0, lastError: null })).toBe(
      'Bluetooth is off.'
    );
  });
});

