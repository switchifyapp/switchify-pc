import { contextBridge, ipcRenderer } from 'electron';
import type { ConnectionDetails, PairedDeviceView, PairingSessionView, PcServerStatus } from '../shared/server-status';
import {
  CREATE_PAIRING_SESSION_CHANNEL,
  DISCONNECT_CLIENTS_CHANNEL,
  GET_CURSOR_OVERLAY_ENABLED_CHANNEL,
  GET_CONNECTION_DETAILS_CHANNEL,
  GET_PAIRED_DEVICES_CHANNEL,
  GET_PAIRING_SESSION_CHANNEL,
  SERVER_STATUS_CHANNEL,
  SET_CURSOR_OVERLAY_ENABLED_CHANNEL
} from '../shared/ipc-channels';

contextBridge.exposeInMainWorld('switchifyPc', {
  appName: 'Switchify PC',
  getServerStatus: (): Promise<PcServerStatus> => ipcRenderer.invoke(SERVER_STATUS_CHANNEL),
  getPairingSession: (): Promise<PairingSessionView | null> => ipcRenderer.invoke(GET_PAIRING_SESSION_CHANNEL),
  createPairingSession: (): Promise<PairingSessionView> => ipcRenderer.invoke(CREATE_PAIRING_SESSION_CHANNEL),
  getConnectionDetails: (): Promise<ConnectionDetails> => ipcRenderer.invoke(GET_CONNECTION_DETAILS_CHANNEL),
  getPairedDevices: (): Promise<PairedDeviceView[]> => ipcRenderer.invoke(GET_PAIRED_DEVICES_CHANNEL),
  disconnectClients: (): Promise<PcServerStatus> => ipcRenderer.invoke(DISCONNECT_CLIENTS_CHANNEL),
  getCursorOverlayEnabled: (): Promise<boolean> => ipcRenderer.invoke(GET_CURSOR_OVERLAY_ENABLED_CHANNEL),
  setCursorOverlayEnabled: (enabled: boolean): Promise<boolean> =>
    ipcRenderer.invoke(SET_CURSOR_OVERLAY_ENABLED_CHANNEL, enabled)
});
