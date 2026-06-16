import { app, Menu, nativeImage, Tray, type NativeImage } from 'electron';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import type { PcControlStatus } from '../shared/server-status';

export type SwitchifyTrayOptions = {
  getStatus: () => PcControlStatus;
  showWindow: () => void;
  openSettings: () => void;
  disconnectClients: () => void;
  quit: () => void;
};

export type SwitchifyTray = {
  update: () => void;
  destroy: () => void;
};

export function createSwitchifyTray(options: SwitchifyTrayOptions): SwitchifyTray {
  const tray = new Tray(createTrayIcon());
  let currentMenu: Menu | null = null;

  const update = (): void => {
    const status = options.getStatus();
    currentMenu = buildTrayMenu(options, status);
    tray.setToolTip(`Switchify PC - ${formatTooltipStatus(status)}`);
    if (process.platform !== 'win32') {
      tray.setContextMenu(currentMenu);
    }
  };

  tray.on('click', options.showWindow);
  if (process.platform === 'win32') {
    tray.on('right-click', () => {
      update();
      if (currentMenu) {
        tray.popUpContextMenu(currentMenu);
      }
    });
  }
  update();

  return {
    update,
    destroy: () => tray.destroy()
  };
}

function buildTrayMenu(options: SwitchifyTrayOptions, status: PcControlStatus): Menu {
  return Menu.buildFromTemplate([
    { label: 'Open Switchify PC', click: options.showWindow },
    { label: 'Settings', click: options.openSettings },
    { label: formatMenuStatus(status), enabled: false },
    { type: 'separator' },
    {
      label: 'Disconnect device',
      enabled: status.connectedClientCount > 0,
      click: options.disconnectClients
    },
    { type: 'separator' },
    { label: 'Quit', click: options.quit }
  ]);
}

function createTrayIcon(): NativeImage {
  const iconPath = app.isPackaged
    ? join(process.resourcesPath, 'icon.png')
    : join(process.cwd(), 'build', 'icon.png');
  const image = existsSync(iconPath)
    ? nativeImage.createFromPath(iconPath)
    : nativeImage.createFromDataURL(
        `data:image/svg+xml;base64,${Buffer.from('<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" rx="7" fill="#111113"/><circle cx="16" cy="16" r="10" fill="none" stroke="#f51622" stroke-width="4"/></svg>').toString('base64')}`
      );
  const trayImage = image.resize({ width: 16, height: 16, quality: 'best' });
  trayImage.setTemplateImage(false);
  return trayImage;
}

function formatTooltipStatus(status: PcControlStatus): string {
  if (status.connectedClientCount > 0) return 'device connected';
  if (status.bluetooth.status === 'ready') return 'Bluetooth ready';
  if (status.bluetooth.status === 'unavailable' || status.bluetooth.status === 'error') return 'Bluetooth unavailable';
  if (status.state === 'error') return 'needs attention';
  if (status.state === 'starting') return 'starting';
  return 'not running';
}

function formatMenuStatus(status: PcControlStatus): string {
  if (status.connectedClientCount > 0) return 'Device connected';
  if (status.bluetooth.status === 'ready') return 'Bluetooth ready';
  if (status.bluetooth.status === 'unavailable' || status.bluetooth.status === 'error') return 'Bluetooth unavailable';
  if (status.state === 'error') return 'Needs attention';
  if (status.state === 'starting') return 'Starting...';
  return 'Not running';
}
