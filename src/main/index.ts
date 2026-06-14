import { app, BrowserWindow, nativeTheme, screen, shell } from 'electron';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { DEFAULT_BLUETOOTH_STATUS } from '../shared/bluetooth-status';
import { WindowsBluetoothTransport } from './bluetooth/bluetooth-transport';
import { ControlService } from './control/control-service';
import { CursorOverlay } from './cursor-overlay';
import { registerCursorOverlayIpc } from './cursor-overlay-ipc';
import { MdnsAdvertiser } from './discovery/mdns-advertiser';
import { DesktopCommandExecutor } from './input/command-executor';
import { LibnutWin32InputAdapter } from './input/libnut-win32-adapter';
import { createPointerMovementProfile } from './input/pointer-profile';
import { CommandAuthValidator } from './pairing/auth';
import { PairingApprovalManager } from './pairing/pairing-approval-manager';
import { registerPairingApprovalIpc } from './pairing/pairing-approval-ipc';
import { JsonPairingStore } from './pairing/pairing-store';
import { PairingManager } from './pairing/pairing-manager';
import { registerServerIpc } from './server-ipc';
import { registerSettingsWindowIpc } from './settings-window-ipc';
import { createSwitchifyTray, type SwitchifyTray } from './tray';
import { registerUpdateIpc } from './updates/update-ipc';
import { UpdateService } from './updates/update-service';
import { PcWebSocketServer } from './websocket/server';

const isDev = Boolean(process.env.ELECTRON_RENDERER_URL);
const windowsAppUserModelId = 'app.switchify.pc';
let pcServer: PcWebSocketServer | null = null;
let controlService: ControlService | null = null;
let mainWindow: BrowserWindow | null = null;
let settingsWindow: BrowserWindow | null = null;
let tray: SwitchifyTray | null = null;
let cursorOverlay: CursorOverlay | null = null;
let mdnsAdvertiser: MdnsAdvertiser | null = null;
let bluetoothTransport: WindowsBluetoothTransport | null = null;
let isQuitting = false;

if (process.platform === 'win32' && app.isPackaged) {
  // Chromium GPU sandboxed child processes can fail to start under the
  // uiAccess packaged executable on Windows.
  app.commandLine.appendSwitch('disable-gpu-sandbox');
}

// Matches --color-bg in src/renderer/styles.css so the window frame paints
// the correct base before the renderer loads.
function shellBackgroundColor(): string {
  return nativeTheme.shouldUseDarkColors ? '#161518' : '#f5f4f7';
}

function titleBarOverlayOptions():
  | { color: string; symbolColor: string; height: number }
  | undefined {
  if (process.platform === 'darwin') return undefined;

  return {
    color: shellBackgroundColor(),
    symbolColor: nativeTheme.shouldUseDarkColors ? '#e6e1e5' : '#1c1b1f',
    height: 44
  };
}

function appIconPath(): string | undefined {
  if (process.platform === 'darwin') return undefined;

  const iconPath = app.isPackaged
    ? join(process.resourcesPath, 'icon.png')
    : join(process.cwd(), 'build', 'icon.png');

  return existsSync(iconPath) ? iconPath : undefined;
}

function createMainWindow(): BrowserWindow {
  const overlayOptions = titleBarOverlayOptions();
  const iconPath = appIconPath();
  const window = new BrowserWindow({
    width: 920,
    height: 640,
    minWidth: 720,
    minHeight: 520,
    title: 'Switchify PC',
    backgroundColor: shellBackgroundColor(),
    show: false,
    titleBarStyle: 'hidden',
    ...(overlayOptions ? { titleBarOverlay: overlayOptions } : {}),
    ...(iconPath ? { icon: iconPath } : {}),
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

  const applyThemeBackground = (): void => {
    if (!window.isDestroyed()) {
      window.setBackgroundColor(shellBackgroundColor());
      const overlayOptions = titleBarOverlayOptions();
      if (overlayOptions) {
        window.setTitleBarOverlay?.(overlayOptions);
      }
    }
  };
  nativeTheme.on('updated', applyThemeBackground);
  window.on('closed', () => {
    nativeTheme.off('updated', applyThemeBackground);
  });

  if (isDev && process.env.ELECTRON_RENDERER_URL) {
    void window.loadURL(process.env.ELECTRON_RENDERER_URL);
  } else {
    void window.loadFile(join(__dirname, '../renderer/index.html'));
  }

  return window;
}

function createSettingsWindow(): BrowserWindow {
  const overlayOptions = titleBarOverlayOptions();
  const iconPath = appIconPath();
  const window = new BrowserWindow({
    width: 560,
    height: 620,
    minWidth: 440,
    minHeight: 460,
    title: 'Settings',
    backgroundColor: shellBackgroundColor(),
    show: false,
    titleBarStyle: 'hidden',
    ...(overlayOptions ? { titleBarOverlay: overlayOptions } : {}),
    ...(iconPath ? { icon: iconPath } : {}),
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
    if (settingsWindow === window) {
      settingsWindow = null;
    }
  });

  const applyThemeBackground = (): void => {
    if (!window.isDestroyed()) {
      window.setBackgroundColor(shellBackgroundColor());
      const overlayOptions = titleBarOverlayOptions();
      if (overlayOptions) {
        window.setTitleBarOverlay?.(overlayOptions);
      }
    }
  };
  nativeTheme.on('updated', applyThemeBackground);
  window.on('closed', () => {
    nativeTheme.off('updated', applyThemeBackground);
  });

  if (isDev && process.env.ELECTRON_RENDERER_URL) {
    const url = new URL(process.env.ELECTRON_RENDERER_URL);
    url.hash = '/settings';
    void window.loadURL(url.toString());
  } else {
    void window.loadFile(join(__dirname, '../renderer/index.html'), { hash: '/settings' });
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

function showSettingsWindow(): void {
  if (!settingsWindow || settingsWindow.isDestroyed()) {
    settingsWindow = createSettingsWindow();
    return;
  }

  if (settingsWindow.isMinimized()) {
    settingsWindow.restore();
  }
  settingsWindow.show();
  settingsWindow.focus();
}

function quitApp(): void {
  app.quit();
}

app.whenReady().then(() => {
  if (process.platform === 'win32') {
    app.setAppUserModelId(windowsAppUserModelId);
  }

  const pairingStore = new JsonPairingStore(join(app.getPath('userData'), 'pairing-state.json'));
  const pairingManager = new PairingManager(pairingStore);
  const pairingApprovalManager = new PairingApprovalManager(pairingStore);
  const inputAdapter = new LibnutWin32InputAdapter((position) => screen.getDisplayNearestPoint(position).scaleFactor);
  cursorOverlay = new CursorOverlay({});
  mdnsAdvertiser = new MdnsAdvertiser({
    getDesktopId: () => pairingManager.getDesktopId(),
    getPort: () => pcServer?.getStatus().port ?? 0
  });
  const commandExecutor = new DesktopCommandExecutor(inputAdapter, cursorOverlay);
  controlService = new ControlService({
    pairingManager,
    pairingApprovalManager,
    authValidator: new CommandAuthValidator(pairingStore),
    getPointerProfile: () => {
      const cursor = inputAdapter.getMousePosition();
      const display = screen.getDisplayNearestPoint(cursor);
      return createPointerMovementProfile({
        cursor,
        display: {
          bounds: display.bounds,
          scaleFactor: display.scaleFactor
        }
      });
    },
    onStatusChange: (status) => {
      tray?.update();
      void syncMdnsAdvertiser(status.state === 'listening');
    },
    onCommand: (command) => commandExecutor.execute(command)
  });
  pcServer = new PcWebSocketServer({
    controlService
  });
  bluetoothTransport = new WindowsBluetoothTransport({
    controlService,
    getDesktopId: () => pairingManager.getDesktopId(),
    displayName: 'Switchify PC',
    onStatusChange: () => {
      tray?.update();
    }
  });
  registerServerIpc(controlService, pairingManager, pairingStore);
  registerCursorOverlayIpc(cursorOverlay);
  registerPairingApprovalIpc(controlService);
  registerSettingsWindowIpc(showSettingsWindow);
  registerUpdateIpc(
    new UpdateService({
      currentVersion: app.getVersion(),
      downloadsPath: app.getPath('downloads'),
      showItemInFolder: (filePath) => shell.showItemInFolder(filePath)
    })
  );
  void pcServer.start();
  void bluetoothTransport.start();

  mainWindow = createMainWindow();
  tray = createSwitchifyTray({
    getStatus: () =>
      controlService?.getStatus() ?? {
        state: 'stopped',
        port: 0,
        connectedClientCount: 0,
        connectedClients: [],
        lastSeenAt: null,
        lastError: null,
        listeners: [],
        bluetooth: DEFAULT_BLUETOOTH_STATUS
      },
    showWindow: showMainWindow,
    openSettings: showSettingsWindow,
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
  const advertiser = mdnsAdvertiser;
  mdnsAdvertiser = null;
  cursorOverlay?.destroy();
  cursorOverlay = null;
  bluetoothTransport?.stop();
  bluetoothTransport = null;
  const server = pcServer;
  pcServer = null;
  controlService = null;
  void Promise.all([advertiser?.stop(), server.stop()]).finally(() => {
    app.quit();
  });
});

async function syncMdnsAdvertiser(shouldRun: boolean): Promise<void> {
  const advertiser = mdnsAdvertiser;
  if (!advertiser) return;

  try {
    if (shouldRun && !advertiser.isRunning()) {
      await advertiser.start();
    } else if (!shouldRun && advertiser.isRunning()) {
      await advertiser.stop();
    }
  } catch (error) {
    console.error('Switchify discovery failed.', error);
  }
}
