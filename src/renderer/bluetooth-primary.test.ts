import { describe, expect, it } from 'vitest';
import { DEFAULT_BLUETOOTH_STATUS, type BluetoothStatus } from '../shared/bluetooth-status';
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
      body: 'Turn on Bluetooth in Windows. Switchify PC will update when it is available.'
    });
  });

  it('keeps adapter off copy when control status is already error', () => {
    expect(
      bluetoothPrimaryCopy('server-error', bluetoothStatus({ status: 'unavailable', reason: 'adapter_off' }))
    ).toMatchObject({
      title: 'Bluetooth needs attention',
      body: 'Turn on Bluetooth in Windows. Switchify PC will update when it is available.'
    });
  });

  it('formats live radio off state with automatic recovery copy', () => {
    expect(
      bluetoothPrimaryCopy(
        'ready-to-pair',
        bluetoothStatus({
          status: 'unavailable',
          reason: 'adapter_off',
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
    ).toEqual({
      title: 'Bluetooth is off',
      body: 'Turn on Bluetooth in Windows. Switchify PC will reconnect automatically.',
      tone: 'error'
    });
  });

  it('formats restarting state when the radio is on', () => {
    expect(
      bluetoothPrimaryCopy(
        'starting',
        bluetoothStatus({
          status: 'starting',
          system: {
            adapterPresent: true,
            radioState: 'on',
            isLowEnergySupported: true,
            isPeripheralRoleSupported: true,
            lastCheckedAt: 1,
            lastChangedAt: 1
          }
        })
      )
    ).toMatchObject({
      title: 'Getting Bluetooth ready...',
      body: 'Switchify PC is restarting nearby device connection.'
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

