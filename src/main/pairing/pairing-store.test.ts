import { describe, expect, it } from 'vitest';
import { toPairedDeviceViews, type PairingState } from './pairing-store';

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
