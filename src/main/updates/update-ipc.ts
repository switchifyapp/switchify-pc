import { ipcMain } from 'electron';
import {
  CHECK_FOR_UPDATES_CHANNEL,
  DOWNLOAD_UPDATE_CHANNEL,
  GET_UPDATE_STATE_CHANNEL,
  INSTALL_DOWNLOADED_UPDATE_CHANNEL
} from '../../shared/ipc-channels';
import type { UpdateService } from './update-service';

export function registerUpdateIpc(updateService: UpdateService): void {
  ipcMain.handle(GET_UPDATE_STATE_CHANNEL, () => updateService.getState());
  ipcMain.handle(CHECK_FOR_UPDATES_CHANNEL, () => updateService.checkForUpdates());
  ipcMain.handle(DOWNLOAD_UPDATE_CHANNEL, () => updateService.downloadUpdate());
  ipcMain.handle(INSTALL_DOWNLOADED_UPDATE_CHANNEL, () => updateService.installDownloadedUpdate());
}
