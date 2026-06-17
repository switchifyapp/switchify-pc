import { ipcMain } from 'electron';
import {
  GET_SYSTEM_STARTUP_SETTINGS_CHANNEL,
  SET_START_WITH_SYSTEM_CHANNEL
} from '../shared/ipc-channels';
import type { SystemStartupService } from './system-startup';

export function registerSystemStartupIpc(systemStartup: SystemStartupService): void {
  ipcMain.handle(GET_SYSTEM_STARTUP_SETTINGS_CHANNEL, () => systemStartup.getSettings());
  ipcMain.handle(SET_START_WITH_SYSTEM_CHANNEL, (_event, enabled: unknown) =>
    systemStartup.setStartWithSystem(Boolean(enabled))
  );
}
