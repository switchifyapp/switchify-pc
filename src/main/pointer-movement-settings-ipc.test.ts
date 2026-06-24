import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
  GET_POINTER_MOVEMENT_SETTINGS_CHANNEL,
  SET_POINTER_MOVEMENT_SETTINGS_CHANNEL
} from '../shared/ipc-channels';
import type { PointerMovementSettings } from '../shared/pointer-movement-settings';
import { registerPointerMovementSettingsIpc } from './pointer-movement-settings-ipc';
import type { JsonPointerMovementSettingsStore } from './pointer-movement-settings-store';

type IpcHandler = (event: Electron.IpcMainInvokeEvent, ...args: unknown[]) => unknown;

const ipcHandlers = new Map<string, IpcHandler>();

vi.mock('electron', () => ({
  ipcMain: {
    handle: vi.fn((channel: string, handler: IpcHandler) => {
      ipcHandlers.set(channel, handler);
    })
  }
}));

describe('registerPointerMovementSettingsIpc', () => {
  beforeEach(() => {
    ipcHandlers.clear();
  });

  it('returns stored settings', async () => {
    const settings = { scalePercent: 125 };
    const store = createStore(settings);

    registerPointerMovementSettingsIpc(store, vi.fn());

    await expect(invoke(GET_POINTER_MOVEMENT_SETTINGS_CHANNEL)).resolves.toEqual(settings);
  });

  it('normalizes and saves settings', async () => {
    const store = createStore();

    registerPointerMovementSettingsIpc(store, vi.fn());

    await expect(invoke(SET_POINTER_MOVEMENT_SETTINGS_CHANNEL, { scalePercent: 123 })).resolves.toEqual({
      scalePercent: 125
    });
    expect(store.save).toHaveBeenCalledWith({ scalePercent: 125 });
  });

  it('notifies when settings change', async () => {
    const store = createStore();
    const onSettingsChanged = vi.fn();

    registerPointerMovementSettingsIpc(store, onSettingsChanged);
    await invoke(SET_POINTER_MOVEMENT_SETTINGS_CHANNEL, { scalePercent: 75 });

    expect(onSettingsChanged).toHaveBeenCalledWith({ scalePercent: 75 });
  });

  it('normalizes migrated percentage settings before saving and notifying', async () => {
    const store = createStore();
    const onSettingsChanged = vi.fn();

    registerPointerMovementSettingsIpc(store, onSettingsChanged);

    await expect(
      invoke(SET_POINTER_MOVEMENT_SETTINGS_CHANNEL, {
        percentages: { small: 9, medium: 24, large: 50 }
      })
    ).resolves.toEqual({ scalePercent: 195 });
    expect(store.save).toHaveBeenCalledWith({ scalePercent: 195 });
    expect(onSettingsChanged).toHaveBeenCalledWith({ scalePercent: 195 });
  });
});

function createStore(settings: PointerMovementSettings = { scalePercent: 100 }): JsonPointerMovementSettingsStore {
  return {
    load: vi.fn(() => settings),
    save: vi.fn((nextSettings: PointerMovementSettings) => nextSettings)
  } as unknown as JsonPointerMovementSettingsStore;
}

function invoke(channel: string, ...args: unknown[]): Promise<unknown> {
  const handler = ipcHandlers.get(channel);
  if (!handler) throw new Error(`Handler was not registered: ${channel}`);
  return Promise.resolve(handler({ sender: {} } as Electron.IpcMainInvokeEvent, ...args));
}
