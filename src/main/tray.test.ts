import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_BLUETOOTH_STATUS } from '../shared/bluetooth-status';
import type { PcControlStatus } from '../shared/server-status';

const electronMock = vi.hoisted(() => {
  type Handler = (...args: unknown[]) => void;
  type MenuTemplateItem = {
    label?: string;
    enabled?: boolean;
    click?: () => void;
    type?: string;
  };

  class FakeTray {
    readonly handlers = new Map<string, Handler[]>();
    readonly setToolTip = vi.fn();
    readonly setContextMenu = vi.fn();
    readonly popUpContextMenu = vi.fn();
    readonly destroy = vi.fn();

    on(event: string, handler: Handler): this {
      const handlers = this.handlers.get(event) ?? [];
      handlers.push(handler);
      this.handlers.set(event, handlers);
      return this;
    }

    emit(event: string, ...args: unknown[]): void {
      for (const handler of this.handlers.get(event) ?? []) {
        handler(...args);
      }
    }
  }

  const trayInstances: FakeTray[] = [];
  const image = {
    resize: vi.fn(() => ({
      setTemplateImage: vi.fn()
    }))
  };
  const buildFromTemplate = vi.fn((template: MenuTemplateItem[]) => ({ template }));

  return {
    FakeTray,
    trayInstances,
    buildFromTemplate,
    image
  };
});

vi.mock('electron', () => ({
  app: {
    isPackaged: false
  },
  Menu: {
    buildFromTemplate: electronMock.buildFromTemplate
  },
  nativeImage: {
    createFromDataURL: vi.fn(() => electronMock.image),
    createFromPath: vi.fn(() => electronMock.image)
  },
  Tray: function Tray() {
    const tray = new electronMock.FakeTray();
    electronMock.trayInstances.push(tray);
    return tray;
  }
}));

import { createSwitchifyTray } from './tray';

describe('createSwitchifyTray', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    electronMock.trayInstances.length = 0;
    electronMock.buildFromTemplate.mockClear();
    electronMock.image.resize.mockClear();
    setPlatform('win32');
  });

  afterEach(() => {
    setPlatform(originalPlatform);
    vi.clearAllMocks();
  });

  it('opens the main window on left-click', () => {
    const callbacks = createCallbacks();
    createSwitchifyTray({
      ...callbacks,
      getStatus: () => status()
    });

    tray().emit('click');

    expect(callbacks.showWindow).toHaveBeenCalledTimes(1);
    expect(callbacks.openSettings).not.toHaveBeenCalled();
    expect(callbacks.disconnectClients).not.toHaveBeenCalled();
    expect(callbacks.quit).not.toHaveBeenCalled();
  });

  it('attaches the context menu on Windows', () => {
    const callbacks = createCallbacks();
    createSwitchifyTray({
      ...callbacks,
      getStatus: () => status()
    });

    expect(tray().setContextMenu).toHaveBeenCalledTimes(1);
    expect(tray().setContextMenu).toHaveBeenCalledWith(lastMenu());
    expect(tray().popUpContextMenu).not.toHaveBeenCalled();
    expect(tray().handlers.has('right-click')).toBe(false);
  });

  it('refreshes the context menu with the latest status on update', () => {
    const callbacks = createCallbacks();
    let connectedClientCount = 0;
    const switchifyTray = createSwitchifyTray({
      ...callbacks,
      getStatus: () => status({ connectedClientCount })
    });

    connectedClientCount = 1;
    switchifyTray.update();

    const disconnectItem = lastMenu().template.find((item) => item.label === 'Disconnect device');
    expect(disconnectItem?.enabled).toBe(true);
    expect(tray().setContextMenu).toHaveBeenLastCalledWith(lastMenu());
  });

  it('refreshes the tooltip on update', () => {
    const callbacks = createCallbacks();
    const switchifyTray = createSwitchifyTray({
      ...callbacks,
      getStatus: () => status({ state: 'starting' })
    });

    switchifyTray.update();

    expect(tray().setToolTip).toHaveBeenLastCalledWith('Switchify PC - starting');
  });

  it('destroys the tray', () => {
    const callbacks = createCallbacks();
    const switchifyTray = createSwitchifyTray({
      ...callbacks,
      getStatus: () => status()
    });

    switchifyTray.destroy();

    expect(tray().destroy).toHaveBeenCalledTimes(1);
  });
});

function createCallbacks(): Omit<Parameters<typeof createSwitchifyTray>[0], 'getStatus'> {
  return {
    showWindow: vi.fn(),
    openSettings: vi.fn(),
    disconnectClients: vi.fn(),
    quit: vi.fn()
  };
}

function status(patch: Partial<PcControlStatus> = {}): PcControlStatus {
  return {
    state: 'ready',
    desktopId: 'desktop-id',
    connectedClientCount: 0,
    connectedClients: [],
    lastSeenAt: null,
    lastError: null,
    bluetooth: DEFAULT_BLUETOOTH_STATUS,
    ...patch
  };
}

function tray(): InstanceType<typeof electronMock.FakeTray> {
  const currentTray = electronMock.trayInstances.at(-1);
  if (!currentTray) {
    throw new Error('Expected a tray instance.');
  }
  return currentTray;
}

function lastMenu(): { template: Array<{ label?: string; enabled?: boolean; click?: () => void; type?: string }> } {
  const calls = electronMock.buildFromTemplate.mock.calls;
  const template = calls.at(-1)?.[0];
  if (!template) {
    throw new Error('Expected a menu template.');
  }
  return { template };
}

function setPlatform(platform: NodeJS.Platform): void {
  Object.defineProperty(process, 'platform', {
    configurable: true,
    value: platform
  });
}
