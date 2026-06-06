import { app, BrowserWindow, screen } from 'electron';
import { join } from 'node:path';
import { CursorOverlay } from './cursor-overlay';
import { registerCursorOverlayIpc } from './cursor-overlay-ipc';
import { DesktopCommandExecutor } from './input/command-executor';
import { LibnutWin32InputAdapter } from './input/libnut-win32-adapter';
import { CommandAuthValidator } from './pairing/auth';
import { JsonPairingStore } from './pairing/pairing-store';
import { PairingManager } from './pairing/pairing-manager';
import { registerServerIpc } from './server-ipc';
import { createSwitchifyTray, type SwitchifyTray } from './tray';
import { PcWebSocketServer } from './websocket/server';

const isDev = Boolean(process.env.ELECTRON_RENDERER_URL);
let pcServer: PcWebSocketServer | null = null;
let mainWindow: BrowserWindow | null = null;
let tray: SwitchifyTray | null = null;
let cursorOverlay: CursorOverlay | null = null;
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

  window.on('close', (event) => {
    if (isQuitting) return;
    event.preventDefault();
    window.hide();
  });

  window.on('closed', () => {
    if (mainWindow === window) {
      mainWindow = null;
    }
  });

  if (isDev && process.env.ELECTRON_RENDERER_URL) {
    void window.loadURL(process.env.ELECTRON_RENDERER_URL);
  } else {
    void window.loadFile(join(__dirname, '../renderer/index.html'));
  }

  return window;
}

function showMainWindow(): void {
  if (!mainWindow || mainWindow.isDestroyed()) {
    mainWindow = createMainWindow();
    return;
  }

  if (mainWindow.isMinimized()) {
    mainWindow.restore();
  }
  mainWindow.show();
  mainWindow.focus();
}

function quitApp(): void {
  app.quit();
}

app.whenReady().then(() => {
  const pairingStore = new JsonPairingStore(join(app.getPath('userData'), 'pairing-state.json'));
  const pairingManager = new PairingManager(pairingStore);
  const inputAdapter = new LibnutWin32InputAdapter((position) => screen.getDisplayNearestPoint(position).scaleFactor);
  cursorOverlay = new CursorOverlay({
    getCursorPosition: () => inputAdapter.getMousePosition()
  });
  const commandExecutor = new DesktopCommandExecutor(inputAdapter, cursorOverlay);
  pcServer = new PcWebSocketServer({
    pairingManager,
    authValidator: new CommandAuthValidator(pairingStore),
    onStatusChange: () => tray?.update(),
    onCommand: (command) => commandExecutor.execute(command)
  });
  registerServerIpc(pcServer, pairingManager, pairingStore);
  registerCursorOverlayIpc(cursorOverlay);
  void pcServer.start();

  mainWindow = createMainWindow();
  tray = createSwitchifyTray({
    getStatus: () =>
      pcServer?.getStatus() ?? {
        state: 'stopped',
        port: 0,
        connectedClientCount: 0,
        connectedClients: [],
        lastSeenAt: null,
        lastError: null
      },
    showWindow: showMainWindow,
    disconnectClients: () => {
      pcServer?.disconnectClients();
      tray?.update();
    },
    quit: quitApp
  });

  app.on('activate', () => {
    showMainWindow();
  });
});

app.on('window-all-closed', () => {});

app.on('before-quit', (event) => {
  if (!pcServer || isQuitting) return;

  event.preventDefault();
  isQuitting = true;
  tray?.destroy();
  tray = null;
  cursorOverlay?.destroy();
  cursorOverlay = null;
  void pcServer.stop().finally(() => {
    app.quit();
  });
});
