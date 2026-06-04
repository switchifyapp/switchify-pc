import { app, BrowserWindow } from 'electron';
import { join } from 'node:path';
import { CommandAuthValidator } from './pairing/auth';
import { JsonPairingStore } from './pairing/pairing-store';
import { PairingManager } from './pairing/pairing-manager';
import { registerServerIpc } from './server-ipc';
import { PcWebSocketServer } from './websocket/server';

const isDev = Boolean(process.env.ELECTRON_RENDERER_URL);
let pcServer: PcWebSocketServer | null = null;
let isQuitting = false;

function createMainWindow(): BrowserWindow {
  const window = new BrowserWindow({
    width: 920,
    height: 640,
    minWidth: 720,
    minHeight: 520,
    title: 'Switchify PC',
    backgroundColor: '#f7f7f2',
    show: false,
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false
    }
  });

  window.once('ready-to-show', () => {
    window.show();
  });

  if (isDev && process.env.ELECTRON_RENDERER_URL) {
    void window.loadURL(process.env.ELECTRON_RENDERER_URL);
  } else {
    void window.loadFile(join(__dirname, '../renderer/index.html'));
  }

  return window;
}

app.whenReady().then(() => {
  const pairingStore = new JsonPairingStore(join(app.getPath('userData'), 'pairing-state.json'));
  const pairingManager = new PairingManager(pairingStore);
  pcServer = new PcWebSocketServer({
    pairingManager,
    authValidator: new CommandAuthValidator(pairingStore)
  });
  registerServerIpc(pcServer);
  void pcServer.start();

  createMainWindow();

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) {
      createMainWindow();
    }
  });
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

app.on('before-quit', (event) => {
  if (!pcServer || isQuitting) return;

  event.preventDefault();
  void pcServer.stop().finally(() => {
    isQuitting = true;
    app.quit();
  });
});
