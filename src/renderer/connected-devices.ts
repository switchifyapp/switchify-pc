import type { PairedDeviceView, PcConnectedClient } from '../shared/server-status';

export type ConnectedDeviceView = {
  connectionId: string;
  deviceName: string;
  remoteAddress: string | null;
  connectedAt: number;
  lastSeenAt: number | null;
};

export function toConnectedDeviceViews(
  connectedClients: PcConnectedClient[],
  pairedDevices: PairedDeviceView[]
): ConnectedDeviceView[] {
  const pairedDeviceNames = new Map(pairedDevices.map((device) => [device.deviceId, device.deviceName]));

  return connectedClients.map((client) => ({
    connectionId: client.id,
    deviceName: client.deviceId ? pairedDeviceNames.get(client.deviceId) ?? 'Connected device' : 'Connected device',
    remoteAddress: client.remoteAddress,
    connectedAt: client.connectedAt,
    lastSeenAt: client.lastSeenAt
  }));
}
