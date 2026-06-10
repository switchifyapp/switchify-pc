import { ipcMain } from 'electron';
import {
  DISCONNECT_CLIENTS_CHANNEL,
  FORGET_PAIRED_DEVICE_CHANNEL,
  GET_CONNECTION_DETAILS_CHANNEL,
  GET_PAIRED_DEVICES_CHANNEL,
  SERVER_STATUS_CHANNEL
} from '../shared/ipc-channels';
import type { ConnectionDetails } from '../shared/server-status';
import { getConnectionCandidates } from './network-addresses';
import type { PairingManager } from './pairing/pairing-manager';
import type { PairingStore } from './pairing/pairing-store';
import { removePairedDevice, toPairedDeviceViews } from './pairing/pairing-store';
import type { PcWebSocketServer } from './websocket/server';

export function registerServerIpc(server: PcWebSocketServer, pairingManager: PairingManager, pairingStore: PairingStore): void {
  ipcMain.handle(SERVER_STATUS_CHANNEL, () => server.getStatus());
  ipcMain.handle(GET_CONNECTION_DETAILS_CHANNEL, async () => {
    const status = server.getStatus();
    const websocketUrls = getConnectionCandidates(status.port).map((candidate) => candidate.websocketUrl);
    return {
      desktopId: await pairingManager.getDesktopId(),
      websocketUrl: websocketUrls[0] ?? `ws://127.0.0.1:${status.port}`,
      websocketUrls
    } satisfies ConnectionDetails;
  });
  ipcMain.handle(GET_PAIRED_DEVICES_CHANNEL, async () => toPairedDeviceViews(await pairingStore.load()));
  ipcMain.handle(DISCONNECT_CLIENTS_CHANNEL, () => server.disconnectClients());
  ipcMain.handle(FORGET_PAIRED_DEVICE_CHANNEL, async (_event, deviceId: unknown) => {
    if (typeof deviceId !== 'string' || deviceId.length === 0) {
      return { ok: false, reason: 'invalid_device_id' };
    }

    const state = await pairingStore.load();
    const exists = state.pairedDevices.some((device) => device.deviceId === deviceId);
    if (!exists) {
      return { ok: false, reason: 'device_not_found' };
    }

    const nextState = removePairedDevice(state, deviceId);
    await pairingStore.save(nextState);

    return {
      ok: true,
      pairedDevices: toPairedDeviceViews(nextState),
      status: server.disconnectDevice(deviceId)
    };
  });
}
