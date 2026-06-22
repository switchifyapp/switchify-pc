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
  type FakeMenu = {
    template: MenuTemplateItem[];
    once: ReturnType<typeof vi.fn>;
    emitMenuWillClose: () => void;
  };

  class FakeTray {
    readonly handlers = new Map<string, Handler[]>();
    readonly setToolTip = vi.fn();
    readonly setContextMenu = vi.fn();
    readonly popUpContextMenu = vi.fn();
    readonly focus = vi.fn();
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
  const menus: FakeMenu[] = [];
  const buildFromTemplate = vi.fn((template: MenuTemplateItem[]) => {
    let menuWillCloseHandler: Handler | null = null;
    const menu = {
      template,
      once: vi.fn((event: string, handler: Handler) => {
        if (event === 'menu-will-close') {
          menuWillCloseHandler = handler;
        }
      }),
      emitMenuWillClose: () => {
        menuWillCloseHandler?.();
      }
    };
    menus.push(menu);
    return menu;
  });

  return {
    FakeTray,
    trayInstances,
    menus,
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
import { trayMenuPosition } from './tray';

describe('createSwitchifyTray', () => {
  const originalPlatform = process.platform;

  beforeEach(() => {
    electronMock.trayInstances.length = 0;
    electronMock.menus.length = 0;
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

  it('uses an anchored manual popup on Windows right-click', () => {
    const callbacks = createCallbacks();
    createSwitchifyTray({
      ...callbacks,
      getStatus: () => status()
    });

    expect(tray().setContextMenu).not.toHaveBeenCalled();
    expect(tray().handlers.has('right-click')).toBe(true);

    tray().emit('right-click', {}, { x: 100, y: 100, width: 16, height: 24 });

    expect(tray().popUpContextMenu).toHaveBeenCalledTimes(1);
    expect(tray().popUpContextMenu).toHaveBeenCalledWith(lastMenu(), { x: 100, y: 124 });
  });

  it('refreshes the context menu with the latest status before Windows popup', () => {
    const callbacks = createCallbacks();
    let connectedClientCount = 0;
    createSwitchifyTray({
      ...callbacks,
      getStatus: () => status({ connectedClientCount })
    });

    connectedClientCount = 1;
    tray().emit('right-click', {}, { x: 100, y: 100, width: 16, height: 24 });

    const disconnectItem = lastMenu().template.find((item) => item.label === 'Disconnect device');
    expect(disconnectItem?.enabled).toBe(true);
    expect(tray().popUpContextMenu).toHaveBeenLastCalledWith(lastMenu(), { x: 100, y: 124 });
  });

  it('restores notification area focus when the Windows popup closes', () => {
    const callbacks = createCallbacks();
    createSwitchifyTray({
      ...callbacks,
      getStatus: () => status()
    });

    tray().emit('right-click', {}, { x: 100, y: 100, width: 16, height: 24 });
    lastMenu().emitMenuWillClose();

    expect(tray().focus).toHaveBeenCalledTimes(1);
  });

  it('attaches the context menu on non-Windows platforms', () => {
    setPlatform('darwin');
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

describe('trayMenuPosition', () => {
  it('anchors a menu below the tray bounds', () => {
    expect(trayMenuPosition({ x: 10.4, y: 20.2, width: 16, height: 18.6 })).toEqual({ x: 10, y: 39 });
  });

  it('returns undefined for missing or invalid bounds', () => {
    expect(trayMenuPosition(undefined)).toBeUndefined();
    expect(trayMenuPosition({ x: Number.NaN, y: 20, width: 16, height: 18 })).toBeUndefined();
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

function lastMenu(): {
  template: Array<{ label?: string; enabled?: boolean; click?: () => void; type?: string }>;
  emitMenuWillClose: () => void;
} {
  const menu = electronMock.menus.at(-1);
  if (!menu) {
    throw new Error('Expected a menu template.');
  }
  return menu;
}

function setPlatform(platform: NodeJS.Platform): void {
  Object.defineProperty(process, 'platform', {
    configurable: true,
    value: platform
  });
}
