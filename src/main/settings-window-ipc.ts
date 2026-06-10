import { ipcMain } from 'electron';
import { OPEN_SETTINGS_WINDOW_CHANNEL } from '../shared/ipc-channels';

export function registerSettingsWindowIpc(openSettingsWindow: () => void): void {
  ipcMain.handle(OPEN_SETTINGS_WINDOW_CHANNEL, () => {
    openSettingsWindow();
  });
}
