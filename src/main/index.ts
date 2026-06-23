import { app, BrowserWindow, nativeTheme, screen } from 'electron';
import { autoUpdater } from 'electron-updater';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { DEFAULT_BLUETOOTH_STATUS } from '../shared/bluetooth-status';
import { SHOW_SETTINGS_SECTION_CHANNEL } from '../shared/ipc-channels';
import type { SettingsSectionId } from '../shared/settings';
import { WindowsBluetoothTransport } from './bluetooth/bluetooth-transport';
import { ControlService } from './control/control-service';
import { CursorOverlay } from './cursor-overlay';
import { registerCursorOverlayIpc } from './cursor-overlay-ipc';
import { JsonCursorOverlaySettingsStore } from './cursor-overlay-settings-store';
import { registerAppWindowIpc } from './app-window-ipc';
import { registerExternalUrlIpc } from './external-url-ipc';
import { DesktopCommandExecutor } from './input/command-executor';
import { LibnutWin32InputAdapter } from './input/libnut-win32-adapter';
import { createPointerMovementProfile } from './input/pointer-profile';
import { CommandAuthValidator } from './pairing/auth';
import { PairingApprovalManager } from './pairing/pairing-approval-manager';
import { registerPairingApprovalIpc } from './pairing/pairing-approval-ipc';
import { JsonPairingStore } from './pairing/pairing-store';
import { PairingManager } from './pairing/pairing-manager';
import { registerPointerMovementSettingsIpc } from './pointer-movement-settings-ipc';
import { JsonPointerMovementSettingsStore } from './pointer-movement-settings-store';
import { registerServerIpc } from './server-ipc';
import { registerSettingsWindowIpc } from './settings-window-ipc';
import { secondInstanceAction } from './single-instance';
import { registerSystemStartupIpc } from './system-startup-ipc';
import { shouldStartHidden, SystemStartupService } from './system-startup';
import { createSwitchifyTray, type SwitchifyTray } from './tray';
import { registerUpdateIpc } from './updates/update-ipc';
import { UpdateService } from './updates/update-service';
import { WINDOWS_APP_USER_MODEL_ID } from './windows-app-user-model-id';

const isDev = Boolean(process.env.ELECTRON_RENDERER_URL);
let controlService: ControlService | null = null;
let mainWindow: BrowserWindow | null = null;
let settingsWindow: BrowserWindow | null = null;
let tray: SwitchifyTray | null = null;
let cursorOverlay: CursorOverlay | null = null;
let bluetoothTransport: WindowsBluetoothTransport | null = null;
let releaseHeldMouseButtons: (() => Promise<void>) | null = null;
let isQuitting = false;

if (process.platform === 'win32' && app.isPackaged) {
  // Chromium GPU sandboxed child processes can fail to start under the
  // uiAccess packaged executable on Windows.
  app.commandLine.appendSwitch('disable-gpu-sandbox');
}

const gotSingleInstanceLock = app.requestSingleInstanceLock();

// Matches --color-bg in src/renderer/styles.css so the window frame paints
// the correct base before the renderer loads.
function shellBackgroundColor(): string {
  return nativeTheme.shouldUseDarkColors ? '#161518' : '#f5f4f7';
}

function appIconPath(): string | undefined {
  if (process.platform === 'darwin') return undefined;

  const iconPath = app.isPackaged
    ? join(process.resourcesPath, 'icon.png')
    : join(process.cwd(), 'build', 'icon.png');

  return existsSync(iconPath) ? iconPath : undefined;
}

type MainWindowOptions = {
  showOnReady?: boolean;
};

function createMainWindow(options: MainWindowOptions = {}): BrowserWindow {
  const showOnReady = options.showOnReady ?? true;
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
    maximizable: false,
    ...(iconPath ? { icon: iconPath } : {}),
    webPreferences: {
      preload: join(__dirname, '../preload/index.js'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false
    }
  });

  window.once('ready-to-show', () => {
    if (showOnReady) {
      window.show();
    }
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

function settingsHashForSection(section: SettingsSectionId): string {
  return section === 'general' ? '/settings' : `/settings/${section}`;
}

function createSettingsWindow(section: SettingsSectionId = 'general'): BrowserWindow {
  const iconPath = appIconPath();
  const window = new BrowserWindow({
    width: 820,
    height: 620,
    minWidth: 680,
    minHeight: 460,
    title: 'Settings',
    backgroundColor: shellBackgroundColor(),
    show: false,
    titleBarStyle: 'hidden',
    minimizable: false,
    maximizable: false,
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
    }
  };
  nativeTheme.on('updated', applyThemeBackground);
  window.on('closed', () => {
    nativeTheme.off('updated', applyThemeBackground);
  });

  if (isDev && process.env.ELECTRON_RENDERER_URL) {
    const url = new URL(process.env.ELECTRON_RENDERER_URL);
    url.hash = settingsHashForSection(section);
    void window.loadURL(url.toString());
  } else {
    void window.loadFile(join(__dirname, '../renderer/index.html'), { hash: settingsHashForSection(section) });
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

function sendSettingsSection(window: BrowserWindow, section: SettingsSectionId): void {
  const send = (): void => {
    if (!window.isDestroyed()) {
      window.webContents.send(SHOW_SETTINGS_SECTION_CHANNEL, section);
    }
  };

  if (window.webContents.isLoading()) {
    window.webContents.once('did-finish-load', send);
    return;
  }

  send();
}

function showSettingsWindow(section: SettingsSectionId = 'general'): void {
  if (!settingsWindow || settingsWindow.isDestroyed()) {
    settingsWindow = createSettingsWindow(section);
    return;
  }

  if (settingsWindow.isMinimized()) {
    settingsWindow.restore();
  }
  settingsWindow.show();
  settingsWindow.focus();
  sendSettingsSection(settingsWindow, section);
}

function quitApp(): void {
  app.quit();
}

if (!gotSingleInstanceLock) {
  app.quit();
} else {
  app.on('second-instance', (_event, argv) => {
    if (secondInstanceAction(argv, process.platform) === 'ignore') {
      return;
    }

    showMainWindow();
  });

  app.whenReady().then(() => {
    if (process.platform === 'win32') {
      app.setAppUserModelId(WINDOWS_APP_USER_MODEL_ID);
    }

    const startHidden = shouldStartHidden(process.argv, process.platform);
    const systemStartup = new SystemStartupService({
      platform: process.platform,
      isPackaged: app.isPackaged,
      executablePath: process.execPath,
      appUserModelId: WINDOWS_APP_USER_MODEL_ID,
      getLoginItemSettings: (options) => app.getLoginItemSettings(options),
      setLoginItemSettings: (settings) => app.setLoginItemSettings(settings)
    });
    const pairingStore = new JsonPairingStore(join(app.getPath('userData'), 'pairing-state.json'));
    const cursorOverlaySettingsStore = new JsonCursorOverlaySettingsStore(
      join(app.getPath('userData'), 'cursor-overlay-settings.json')
    );
    const pointerMovementSettingsStore = new JsonPointerMovementSettingsStore(
      join(app.getPath('userData'), 'pointer-movement-settings.json')
    );
    let pointerMovementSettings = pointerMovementSettingsStore.load();
    const pairingManager = new PairingManager(pairingStore);
    const pairingApprovalManager = new PairingApprovalManager(pairingStore);
    const inputAdapter = new LibnutWin32InputAdapter((position) => {
      const display = screen.getDisplayNearestPoint(position);
      return {
        bounds: display.bounds,
        scaleFactor: display.scaleFactor
      };
    }, undefined, pointerMovementSettings);
    cursorOverlay = new CursorOverlay({ settings: cursorOverlaySettingsStore.load() });
    const commandExecutor = new DesktopCommandExecutor(inputAdapter, cursorOverlay);
    releaseHeldMouseButtons = () => commandExecutor.releaseHeldMouseButtons();
    const releaseHeldMouseButtonsSafely = (): void => {
      void commandExecutor.releaseHeldMouseButtons().catch((error) => {
        console.warn(error instanceof Error ? error.message : 'Could not release held mouse buttons.');
      });
    };
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
          },
          movementSettings: pointerMovementSettings
        });
      },
      onStatusChange: (status) => {
        if (status.connectedClientCount === 0) {
          releaseHeldMouseButtonsSafely();
          cursorOverlay?.endControlSession();
        }
        tray?.update();
      },
      onClientDisconnecting: (connectionId) => {
        bluetoothTransport?.markClientRequestedDisconnect(connectionId);
      },
      onCommand: (command) => commandExecutor.execute(command)
    });
    void pairingManager.getDesktopId().then((desktopId) => {
      controlService?.setDesktopId(desktopId);
    });
    bluetoothTransport = new WindowsBluetoothTransport({
      controlService,
      getDesktopId: () => pairingManager.getDesktopId(),
      displayName: 'Switchify PC',
      onStatusChange: () => {
        tray?.update();
      }
    });
    registerServerIpc(controlService, pairingStore);
    registerCursorOverlayIpc(cursorOverlay, cursorOverlaySettingsStore);
    registerPointerMovementSettingsIpc(pointerMovementSettingsStore, (settings) => {
      pointerMovementSettings = settings;
      inputAdapter.setPointerMovementSettings(settings);
    });
    registerPairingApprovalIpc(controlService);
    registerSettingsWindowIpc(showSettingsWindow);
    registerAppWindowIpc();
    registerExternalUrlIpc();
    registerSystemStartupIpc(systemStartup);
    registerUpdateIpc(
      new UpdateService({
        currentVersion: app.getVersion(),
        isPackaged: app.isPackaged,
        platform: process.platform,
        autoUpdater
      })
    );
    void bluetoothTransport.start();

    mainWindow = createMainWindow({ showOnReady: !startHidden });
    tray = createSwitchifyTray({
      getStatus: () =>
        controlService?.getStatus() ?? {
          state: 'stopped',
          desktopId: null,
          connectedClientCount: 0,
          connectedClients: [],
          lastSeenAt: null,
          lastError: null,
          bluetooth: DEFAULT_BLUETOOTH_STATUS
        },
      showWindow: showMainWindow,
      openSettings: showSettingsWindow,
      disconnectClients: () => {
        releaseHeldMouseButtonsSafely();
        controlService?.disconnectClients();
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
    if (isQuitting) return;

    event.preventDefault();
    isQuitting = true;
    void (releaseHeldMouseButtons?.() ?? Promise.resolve())
      .catch((error) => {
        console.warn(error instanceof Error ? error.message : 'Could not release held mouse buttons.');
      })
      .then(() => {
        tray?.destroy();
        tray = null;
        cursorOverlay?.destroy();
        cursorOverlay = null;
        bluetoothTransport?.stop();
        bluetoothTransport = null;
        releaseHeldMouseButtons = null;
        controlService = null;
      })
      .finally(() => {
        app.quit();
      });
  });
}
