import { describe, expect, it } from 'vitest';
import type { PairedDeviceView, PcConnectedClient } from '../shared/server-status';
import { toConnectedDeviceViews } from './connected-devices';

describe('toConnectedDeviceViews', () => {
  it('uses the saved paired device name for a connected client', () => {
    expect(toConnectedDeviceViews([connectedClient({ deviceId: 'device-1' })], [pairedDevice()])).toEqual([
      {
        connectionId: 'connection-1',
        deviceName: 'Kitchen switch',
        remoteAddress: '192.168.1.10',
        connectedAt: 100,
        lastSeenAt: 200,
        transport: 'websocket'
      }
    ]);
  });

  it('falls back to a generic connected device label when the client has no device id', () => {
    expect(toConnectedDeviceViews([connectedClient({ deviceId: null })], [pairedDevice()])).toEqual([
      expect.objectContaining({
        connectionId: 'connection-1',
        deviceName: 'Connected device'
      })
    ]);
  });

  it('falls back to a generic connected device label when no saved device matches', () => {
    expect(toConnectedDeviceViews([connectedClient({ deviceId: 'unknown-device' })], [pairedDevice()])).toEqual([
      expect.objectContaining({
        connectionId: 'connection-1',
        deviceName: 'Connected device'
      })
    ]);
  });

  it('does not expose raw device ids in connected device view objects', () => {
    const [device] = toConnectedDeviceViews([connectedClient({ deviceId: 'device-1' })], [pairedDevice()]);

    expect(device).not.toHaveProperty('deviceId');
  });
});

function connectedClient(overrides: Partial<PcConnectedClient> = {}): PcConnectedClient {
  return {
    id: 'connection-1',
    deviceId: 'device-1',
    remoteAddress: '192.168.1.10',
    connectedAt: 100,
    lastSeenAt: 200,
    transport: 'websocket',
    ...overrides
  };
}

function pairedDevice(overrides: Partial<PairedDeviceView> = {}): PairedDeviceView {
  return {
    deviceId: 'device-1',
    deviceName: 'Kitchen switch',
    pairedAt: 50,
    lastSeenAt: 75,
    ...overrides
  };
}
