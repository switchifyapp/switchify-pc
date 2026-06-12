import { contextBridge, ipcRenderer } from 'electron';
import type { PairingApprovalDecision, PendingPairingApprovalView } from '../shared/pairing-approval';
import type { ConnectionDetails, PairedDeviceView, PcServerStatus } from '../shared/server-status';
import {
  CHECK_FOR_UPDATES_CHANNEL,
  DISCONNECT_CLIENTS_CHANNEL,
  DOWNLOAD_UPDATE_CHANNEL,
  FORGET_PAIRED_DEVICE_CHANNEL,
  GET_CURSOR_OVERLAY_ENABLED_CHANNEL,
  GET_CONNECTION_DETAILS_CHANNEL,
  GET_PAIRED_DEVICES_CHANNEL,
  GET_PENDING_PAIRING_REQUESTS_CHANNEL,
  GET_UPDATE_STATE_CHANNEL,
  OPEN_SETTINGS_WINDOW_CHANNEL,
  RESPOND_TO_PAIRING_REQUEST_CHANNEL,
  SERVER_STATUS_CHANNEL,
  SET_CURSOR_OVERLAY_ENABLED_CHANNEL,
  SHOW_DOWNLOADED_UPDATE_CHANNEL
} from '../shared/ipc-channels';
import type { UpdateState } from '../shared/update';

contextBridge.exposeInMainWorld('switchifyPc', {
  appName: 'Switchify PC',
  getServerStatus: (): Promise<PcServerStatus> => ipcRenderer.invoke(SERVER_STATUS_CHANNEL),
  getConnectionDetails: (): Promise<ConnectionDetails> => ipcRenderer.invoke(GET_CONNECTION_DETAILS_CHANNEL),
  getPairedDevices: (): Promise<PairedDeviceView[]> => ipcRenderer.invoke(GET_PAIRED_DEVICES_CHANNEL),
  disconnectClients: (): Promise<PcServerStatus> => ipcRenderer.invoke(DISCONNECT_CLIENTS_CHANNEL),
  forgetPairedDevice: (
    deviceId: string
  ): Promise<
    { ok: true; pairedDevices: PairedDeviceView[]; status: PcServerStatus } | { ok: false; reason: string }
  > => ipcRenderer.invoke(FORGET_PAIRED_DEVICE_CHANNEL, deviceId),
  getCursorOverlayEnabled: (): Promise<boolean> => ipcRenderer.invoke(GET_CURSOR_OVERLAY_ENABLED_CHANNEL),
  setCursorOverlayEnabled: (enabled: boolean): Promise<boolean> =>
    ipcRenderer.invoke(SET_CURSOR_OVERLAY_ENABLED_CHANNEL, enabled),
  openSettingsWindow: (): Promise<void> => ipcRenderer.invoke(OPEN_SETTINGS_WINDOW_CHANNEL),
  getPendingPairingRequests: (): Promise<PendingPairingApprovalView[]> =>
    ipcRenderer.invoke(GET_PENDING_PAIRING_REQUESTS_CHANNEL),
  respondToPairingRequest: (requestId: string, decision: PairingApprovalDecision): Promise<{ ok: boolean; reason?: string }> =>
    ipcRenderer.invoke(RESPOND_TO_PAIRING_REQUEST_CHANNEL, requestId, decision),
  getUpdateState: (): Promise<UpdateState> => ipcRenderer.invoke(GET_UPDATE_STATE_CHANNEL),
  checkForUpdates: (): Promise<UpdateState> => ipcRenderer.invoke(CHECK_FOR_UPDATES_CHANNEL),
  downloadUpdate: (): Promise<UpdateState> => ipcRenderer.invoke(DOWNLOAD_UPDATE_CHANNEL),
  showDownloadedUpdate: (): Promise<{ ok: boolean; reason?: string }> =>
    ipcRenderer.invoke(SHOW_DOWNLOADED_UPDATE_CHANNEL)
});
