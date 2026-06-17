import { BrowserWindow, ipcMain } from 'electron';
import {
  APP_WINDOW_CLOSE_CHANNEL,
  APP_WINDOW_MINIMIZE_CHANNEL,
  APP_WINDOW_TOGGLE_MAXIMIZE_CHANNEL
} from '../shared/ipc-channels';

function getSenderWindow(event: Electron.IpcMainInvokeEvent): BrowserWindow | null {
  return BrowserWindow.fromWebContents(event.sender);
}

export function registerAppWindowIpc(): void {
  ipcMain.handle(APP_WINDOW_MINIMIZE_CHANNEL, (event) => {
    getSenderWindow(event)?.minimize();
  });

  ipcMain.handle(APP_WINDOW_TOGGLE_MAXIMIZE_CHANNEL, (event) => {
    const window = getSenderWindow(event);
    if (!window) return;

    if (window.isMaximized()) {
      window.unmaximize();
    } else {
      window.maximize();
    }
  });

  ipcMain.handle(APP_WINDOW_CLOSE_CHANNEL, (event) => {
    getSenderWindow(event)?.close();
  });
}
