import { app, Menu, nativeImage, Tray, type NativeImage } from 'electron';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import type { PcServerStatus } from '../shared/server-status';

export type SwitchifyTrayOptions = {
  getStatus: () => PcServerStatus;
  showWindow: () => void;
  disconnectClients: () => void;
  quit: () => void;
};

export type SwitchifyTray = {
  update: () => void;
  destroy: () => void;
};

export function createSwitchifyTray(options: SwitchifyTrayOptions): SwitchifyTray {
  const tray = new Tray(createTrayIcon());

  const update = (): void => {
    const status = options.getStatus();
    tray.setToolTip(`Switchify PC - ${formatTooltipStatus(status)}`);
    tray.setContextMenu(
      Menu.buildFromTemplate([
        { label: 'Open Switchify PC', click: options.showWindow },
        { label: formatMenuStatus(status), enabled: false },
        { type: 'separator' },
        {
          label: 'Disconnect phone',
          enabled: status.connectedClientCount > 0,
          click: options.disconnectClients
        },
        { type: 'separator' },
        { label: 'Quit', click: options.quit }
      ])
    );
  };

  tray.on('click', options.showWindow);
  update();

  return {
    update,
    destroy: () => tray.destroy()
  };
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

function formatTooltipStatus(status: PcServerStatus): string {
  if (status.connectedClientCount > 0) return 'phone connected';
  if (status.state === 'listening') return 'ready';
  if (status.state === 'error') return 'needs attention';
  if (status.state === 'starting') return 'starting';
  return 'not running';
}

function formatMenuStatus(status: PcServerStatus): string {
  if (status.connectedClientCount > 0) return 'Phone connected';
  if (status.state === 'listening') return 'Ready to connect';
  if (status.state === 'error') return 'Needs attention';
  if (status.state === 'starting') return 'Starting...';
  return 'Not running';
}
