import { BrowserWindow, dialog, ipcMain, type MessageBoxOptions } from 'electron';
import {
  CHECK_FOR_UPDATES_CHANNEL,
  DOWNLOAD_UPDATE_CHANNEL,
  GET_UPDATE_STATE_CHANNEL,
  INSTALL_DOWNLOADED_UPDATE_CHANNEL
} from '../../shared/ipc-channels';
import type { UpdateService } from './update-service';

export const UPDATE_INSTALL_CONFIRMATION_OPTIONS: MessageBoxOptions = {
  type: 'warning',
  title: 'Install update?',
  message: 'Install the downloaded Switchify PC update?',
  detail:
    'Switchify PC will close while the update installer runs. If you rely on Switchify to control this computer, you may temporarily lose access until the app starts again. Make sure you have another way to regain access before continuing.',
  buttons: ['Install update', 'Cancel'],
  defaultId: 1,
  cancelId: 1,
  noLink: true
};

export type UpdateInstallConfirmation = (event: Electron.IpcMainInvokeEvent) => Promise<boolean>;

export function isInstallUpdateConfirmed(response: number): boolean {
  return response === 0;
}

async function showNativeUpdateInstallConfirmation(event: Electron.IpcMainInvokeEvent): Promise<boolean> {
  const parentWindow = BrowserWindow.fromWebContents(event.sender) ?? undefined;
  const result = parentWindow
    ? await dialog.showMessageBox(parentWindow, UPDATE_INSTALL_CONFIRMATION_OPTIONS)
    : await dialog.showMessageBox(UPDATE_INSTALL_CONFIRMATION_OPTIONS);

  return isInstallUpdateConfirmed(result.response);
}

export function registerUpdateIpc(
  updateService: UpdateService,
  options: { confirmInstallDownloadedUpdate?: UpdateInstallConfirmation } = {}
): void {
  const confirmInstallDownloadedUpdate = options.confirmInstallDownloadedUpdate ?? showNativeUpdateInstallConfirmation;

  ipcMain.handle(GET_UPDATE_STATE_CHANNEL, () => updateService.getState());
  ipcMain.handle(CHECK_FOR_UPDATES_CHANNEL, () => updateService.checkForUpdates());
  ipcMain.handle(DOWNLOAD_UPDATE_CHANNEL, () => updateService.downloadUpdate());
  ipcMain.handle(INSTALL_DOWNLOADED_UPDATE_CHANNEL, async (event) => {
    if (updateService.getState().download.status !== 'downloaded') {
      return updateService.installDownloadedUpdate();
    }

    if (!(await confirmInstallDownloadedUpdate(event))) {
      return { ok: false, reason: 'cancelled' };
    }

    return updateService.installDownloadedUpdate();
  });
}
