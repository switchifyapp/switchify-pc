import { contextBridge } from 'electron';

contextBridge.exposeInMainWorld('switchifyPc', {
  appName: 'Switchify PC',
  status: 'Desktop companion scaffold ready.'
});
