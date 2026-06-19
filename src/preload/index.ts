import { contextBridge, ipcRenderer, type IpcRendererEvent } from 'electron';
import type { PairingApprovalDecision, PendingPairingApprovalView } from '../shared/pairing-approval';
import type { PairedDeviceView, PcControlStatus } from '../shared/server-status';
import type { CursorOverlaySettings } from '../shared/cursor-overlay-settings';
import {
  APP_WINDOW_CLOSE_CHANNEL,
  APP_WINDOW_MINIMIZE_CHANNEL,
  CHECK_FOR_UPDATES_CHANNEL,
  DISCONNECT_CLIENTS_CHANNEL,
  DOWNLOAD_UPDATE_CHANNEL,
  FORGET_PAIRED_DEVICE_CHANNEL,
  GET_CURSOR_OVERLAY_ENABLED_CHANNEL,
  GET_CURSOR_OVERLAY_SETTINGS_CHANNEL,
  GET_PAIRED_DEVICES_CHANNEL,
  GET_PENDING_PAIRING_REQUESTS_CHANNEL,
  GET_SYSTEM_STARTUP_SETTINGS_CHANNEL,
  GET_UPDATE_STATE_CHANNEL,
  INSTALL_DOWNLOADED_UPDATE_CHANNEL,
  OPEN_EXTERNAL_URL_CHANNEL,
  OPEN_SETTINGS_WINDOW_CHANNEL,
  RESPOND_TO_PAIRING_REQUEST_CHANNEL,
  SERVER_STATUS_CHANNEL,
  SET_CURSOR_OVERLAY_ENABLED_CHANNEL,
  SET_CURSOR_OVERLAY_SETTINGS_CHANNEL,
  SET_START_WITH_SYSTEM_CHANNEL,
  SHOW_SETTINGS_SECTION_CHANNEL,
} from '../shared/ipc-channels';
import type { SettingsSectionId } from '../shared/settings';
import type { SystemStartupSettings } from '../shared/system-startup';
import type { UpdateState } from '../shared/update';

contextBridge.exposeInMainWorld('switchifyPc', {
  appName: 'Switchify PC',
  minimizeWindow: (): Promise<void> => ipcRenderer.invoke(APP_WINDOW_MINIMIZE_CHANNEL),
  closeWindow: (): Promise<void> => ipcRenderer.invoke(APP_WINDOW_CLOSE_CHANNEL),
  getServerStatus: (): Promise<PcControlStatus> => ipcRenderer.invoke(SERVER_STATUS_CHANNEL),
  getPairedDevices: (): Promise<PairedDeviceView[]> => ipcRenderer.invoke(GET_PAIRED_DEVICES_CHANNEL),
  disconnectClients: (): Promise<PcControlStatus> => ipcRenderer.invoke(DISCONNECT_CLIENTS_CHANNEL),
  forgetPairedDevice: (
    deviceId: string
  ): Promise<
    { ok: true; pairedDevices: PairedDeviceView[]; status: PcControlStatus } | { ok: false; reason: string }
  > => ipcRenderer.invoke(FORGET_PAIRED_DEVICE_CHANNEL, deviceId),
  getCursorOverlayEnabled: (): Promise<boolean> => ipcRenderer.invoke(GET_CURSOR_OVERLAY_ENABLED_CHANNEL),
  setCursorOverlayEnabled: (enabled: boolean): Promise<boolean> =>
    ipcRenderer.invoke(SET_CURSOR_OVERLAY_ENABLED_CHANNEL, enabled),
  getCursorOverlaySettings: (): Promise<CursorOverlaySettings> => ipcRenderer.invoke(GET_CURSOR_OVERLAY_SETTINGS_CHANNEL),
  setCursorOverlaySettings: (settings: CursorOverlaySettings): Promise<CursorOverlaySettings> =>
    ipcRenderer.invoke(SET_CURSOR_OVERLAY_SETTINGS_CHANNEL, settings),
  openSettingsWindow: (section?: SettingsSectionId): Promise<void> =>
    ipcRenderer.invoke(OPEN_SETTINGS_WINDOW_CHANNEL, section),
  onShowSettingsSection: (handler: (section: SettingsSectionId) => void): (() => void) => {
    const listener = (_event: IpcRendererEvent, section: SettingsSectionId): void => handler(section);
    ipcRenderer.on(SHOW_SETTINGS_SECTION_CHANNEL, listener);
    return () => ipcRenderer.removeListener(SHOW_SETTINGS_SECTION_CHANNEL, listener);
  },
  openExternalUrl: (url: string): Promise<{ ok: boolean }> =>
    ipcRenderer.invoke(OPEN_EXTERNAL_URL_CHANNEL, url),
  getPendingPairingRequests: (): Promise<PendingPairingApprovalView[]> =>
    ipcRenderer.invoke(GET_PENDING_PAIRING_REQUESTS_CHANNEL),
  respondToPairingRequest: (requestId: string, decision: PairingApprovalDecision): Promise<{ ok: boolean; reason?: string }> =>
    ipcRenderer.invoke(RESPOND_TO_PAIRING_REQUEST_CHANNEL, requestId, decision),
  getUpdateState: (): Promise<UpdateState> => ipcRenderer.invoke(GET_UPDATE_STATE_CHANNEL),
  checkForUpdates: (): Promise<UpdateState> => ipcRenderer.invoke(CHECK_FOR_UPDATES_CHANNEL),
  downloadUpdate: (): Promise<UpdateState> => ipcRenderer.invoke(DOWNLOAD_UPDATE_CHANNEL),
  getSystemStartupSettings: (): Promise<SystemStartupSettings> =>
    ipcRenderer.invoke(GET_SYSTEM_STARTUP_SETTINGS_CHANNEL),
  setStartWithSystem: (enabled: boolean): Promise<SystemStartupSettings> =>
    ipcRenderer.invoke(SET_START_WITH_SYSTEM_CHANNEL, enabled),
  installDownloadedUpdate: (): Promise<{ ok: boolean; reason?: string }> =>
    ipcRenderer.invoke(INSTALL_DOWNLOADED_UPDATE_CHANNEL)
});
