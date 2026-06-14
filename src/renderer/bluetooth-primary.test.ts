import { describe, expect, it } from 'vitest';
import type { BluetoothStatus } from '../shared/bluetooth-status';
import { bluetoothPrimaryCopy } from './bluetooth-primary';

describe('bluetoothPrimaryCopy', () => {
  it('formats ready Bluetooth state without manual connection language', () => {
    expect(bluetoothPrimaryCopy('ready-to-pair', bluetoothStatus({ status: 'ready' }))).toEqual({
      title: 'Ready for Bluetooth',
      body: 'Open Switchify on your Android device while you are near this PC.',
      tone: 'ready'
    });
  });

  it('formats starting state', () => {
    expect(bluetoothPrimaryCopy('starting', bluetoothStatus({ status: 'starting' })).title).toBe('Getting Bluetooth ready...');
  });

  it('formats adapter off state', () => {
    expect(
      bluetoothPrimaryCopy('ready-to-pair', bluetoothStatus({ status: 'unavailable', reason: 'adapter_off' }))
    ).toMatchObject({
      title: 'Bluetooth needs attention',
      body: 'Turn on Bluetooth on this PC, then refresh.'
    });
  });

  it('keeps adapter off copy when control status is already error', () => {
    expect(
      bluetoothPrimaryCopy('server-error', bluetoothStatus({ status: 'unavailable', reason: 'adapter_off' }))
    ).toMatchObject({
      title: 'Bluetooth needs attention',
      body: 'Turn on Bluetooth on this PC, then refresh.'
    });
  });

  it('formats permission denied state', () => {
    expect(
      bluetoothPrimaryCopy('ready-to-pair', bluetoothStatus({
        status: 'unavailable',
        reason: 'permission_denied'
      }))
    ).toMatchObject({
      title: 'Bluetooth permission denied',
      body: 'Allow Bluetooth access for Switchify PC, then restart the app.'
    });
  });

  it('formats connected state', () => {
    expect(bluetoothPrimaryCopy('connected', bluetoothStatus({ status: 'connected', connectedClientCount: 1 }))).toMatchObject({
      title: 'Your device is connected',
      body: 'You can control this PC from Switchify over Bluetooth.'
    });
  });

  it('does not include QR or local-network copy', () => {
    const copy = bluetoothPrimaryCopy('ready-to-pair', bluetoothStatus({ status: 'ready' }));

    expect(`${copy.title} ${copy.body}`).not.toMatch(/QR|manual|local network|address/i);
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

