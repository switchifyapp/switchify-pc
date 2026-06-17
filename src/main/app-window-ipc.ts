import { BrowserWindow, ipcMain } from 'electron';
import { APP_WINDOW_CLOSE_CHANNEL, APP_WINDOW_MINIMIZE_CHANNEL } from '../shared/ipc-channels';

function getSenderWindow(event: Electron.IpcMainInvokeEvent): BrowserWindow | null {
  return BrowserWindow.fromWebContents(event.sender);
}

export function registerAppWindowIpc(): void {
  ipcMain.handle(APP_WINDOW_MINIMIZE_CHANNEL, (event) => {
    getSenderWindow(event)?.minimize();
  });

  ipcMain.handle(APP_WINDOW_CLOSE_CHANNEL, (event) => {
    getSenderWindow(event)?.close();
  });
}
