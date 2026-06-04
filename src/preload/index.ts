import { contextBridge, ipcRenderer } from 'electron';
import type { PcServerStatus } from '../shared/server-status';
import { SERVER_STATUS_CHANNEL } from '../shared/ipc-channels';

contextBridge.exposeInMainWorld('switchifyPc', {
  appName: 'Switchify PC',
  status: 'Desktop companion scaffold ready.',
  getServerStatus: (): Promise<PcServerStatus> => ipcRenderer.invoke(SERVER_STATUS_CHANNEL)
});
