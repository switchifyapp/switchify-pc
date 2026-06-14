import { ipcMain } from 'electron';
import {
  DISCONNECT_CLIENTS_CHANNEL,
  FORGET_PAIRED_DEVICE_CHANNEL,
  GET_PAIRED_DEVICES_CHANNEL,
  SERVER_STATUS_CHANNEL
} from '../shared/ipc-channels';
import type { ControlService } from './control/control-service';
import type { PairingStore } from './pairing/pairing-store';
import { removePairedDevice, toPairedDeviceViews } from './pairing/pairing-store';

export function registerServerIpc(controlService: ControlService, pairingStore: PairingStore): void {
  ipcMain.handle(SERVER_STATUS_CHANNEL, () => controlService.getStatus());
  ipcMain.handle(GET_PAIRED_DEVICES_CHANNEL, async () => toPairedDeviceViews(await pairingStore.load()));
  ipcMain.handle(DISCONNECT_CLIENTS_CHANNEL, () => controlService.disconnectClients());
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
      status: controlService.disconnectDevice(deviceId)
    };
  });
}
