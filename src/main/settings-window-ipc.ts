import { ipcMain } from 'electron';
import { OPEN_SETTINGS_WINDOW_CHANNEL } from '../shared/ipc-channels';
import { isSettingsSectionId, type SettingsSectionId } from '../shared/settings';

export function registerSettingsWindowIpc(openSettingsWindow: (section?: SettingsSectionId) => void): void {
  ipcMain.handle(OPEN_SETTINGS_WINDOW_CHANNEL, (_event, section) => {
    openSettingsWindow(isSettingsSectionId(section) ? section : undefined);
  });
}
