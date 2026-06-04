import { ipcMain } from 'electron';
import { SERVER_STATUS_CHANNEL } from '../shared/ipc-channels';
import type { PcWebSocketServer } from './websocket/server';

export function registerServerIpc(server: PcWebSocketServer): void {
  ipcMain.handle(SERVER_STATUS_CHANNEL, () => server.getStatus());
}
