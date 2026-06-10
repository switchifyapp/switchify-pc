import { describe, expect, it } from 'vitest';
import { removePairedDevice, toPairedDeviceViews, type PairingState } from './pairing-store';

describe('toPairedDeviceViews', () => {
  it('removes shared tokens from paired device metadata', () => {
    const state = {
      desktopId: 'desktop-1',
      pairedDevices: [
        {
          deviceId: 'android-1',
          deviceName: 'Android device',
          token: 'secret-token',
          pairedAt: 1_000,
          lastSeenAt: 2_000
        }
      ]
    } satisfies PairingState;

    const views = toPairedDeviceViews(state);

    expect(views).toEqual([
      {
        deviceId: 'android-1',
        deviceName: 'Android device',
        pairedAt: 1_000,
        lastSeenAt: 2_000
      }
    ]);
    expect(JSON.stringify(views)).not.toContain('secret-token');
  });
});

describe('removePairedDevice', () => {
  it('removes only the matching paired device and preserves desktop id', () => {
    const state = createState();

    const nextState = removePairedDevice(state, 'android-1');

    expect(nextState.desktopId).toBe('desktop-1');
    expect(nextState.pairedDevices).toEqual([
      {
        deviceId: 'android-2',
        deviceName: 'Tablet',
        token: 'secret-token-2',
        pairedAt: 3_000,
        lastSeenAt: null
      }
    ]);
    expect(state.pairedDevices).toHaveLength(2);
  });

  it('leaves paired devices unchanged when the device id is missing', () => {
    const state = createState();

    const nextState = removePairedDevice(state, 'missing');

    expect(nextState).toEqual(state);
    expect(nextState).not.toBe(state);
    expect(nextState.pairedDevices).not.toBe(state.pairedDevices);
  });
});

function createState(): PairingState {
  return {
    desktopId: 'desktop-1',
    pairedDevices: [
      {
        deviceId: 'android-1',
        deviceName: 'Phone',
        token: 'secret-token-1',
        pairedAt: 1_000,
        lastSeenAt: 2_000
      },
      {
        deviceId: 'android-2',
        deviceName: 'Tablet',
        token: 'secret-token-2',
        pairedAt: 3_000,
        lastSeenAt: null
      }
    ]
  };
}
