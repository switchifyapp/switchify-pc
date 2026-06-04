import { Menu, nativeImage, Tray, type NativeImage } from 'electron';
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
    tray.setToolTip(`Switchify PC - ${formatStatus(status)}`);
    tray.setContextMenu(
      Menu.buildFromTemplate([
        { label: 'Show window', click: options.showWindow },
        { label: formatStatus(status), enabled: false },
        { label: `Connected devices: ${status.connectedClientCount}`, enabled: false },
        { type: 'separator' },
        {
          label: 'Disconnect all',
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
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32"><rect width="32" height="32" rx="7" fill="#1f3d2b"/><path d="M9 11h9.5a4.5 4.5 0 0 1 0 9H13" fill="none" stroke="#f8faf3" stroke-width="3" stroke-linecap="round"/><path d="M14 7 8 11l6 4" fill="none" stroke="#f8faf3" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/><path d="M18 17h5" stroke="#88c36f" stroke-width="3" stroke-linecap="round"/></svg>`;
  const image = nativeImage.createFromDataURL(`data:image/svg+xml;base64,${Buffer.from(svg).toString('base64')}`);
  image.setTemplateImage(false);
  return image;
}

function formatStatus(status: PcServerStatus): string {
  if (status.state === 'listening') return `Listening on port ${status.port}`;
  if (status.state === 'error') return 'Server error';
  return `${status.state.charAt(0).toUpperCase()}${status.state.slice(1)}`;
}
