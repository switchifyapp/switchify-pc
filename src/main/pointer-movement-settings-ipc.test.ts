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
    const settings = { percentages: { small: 3, medium: 12, large: 30 } };
    const store = createStore(settings);

    registerPointerMovementSettingsIpc(store, vi.fn());

    await expect(invoke(GET_POINTER_MOVEMENT_SETTINGS_CHANNEL)).resolves.toEqual(settings);
  });

  it('normalizes and saves settings', async () => {
    const store = createStore();

    registerPointerMovementSettingsIpc(store, vi.fn());

    await expect(
      invoke(SET_POINTER_MOVEMENT_SETTINGS_CHANNEL, {
        percentages: { small: 0.2, medium: 12.3, large: 100 }
      })
    ).resolves.toEqual({
      percentages: {
        small: 1,
        medium: 12.5,
        large: 50
      }
    });
    expect(store.save).toHaveBeenCalledWith({
      percentages: {
        small: 1,
        medium: 12.5,
        large: 50
      }
    });
  });

  it('notifies when settings change', async () => {
    const store = createStore();
    const onSettingsChanged = vi.fn();

    registerPointerMovementSettingsIpc(store, onSettingsChanged);
    await invoke(SET_POINTER_MOVEMENT_SETTINGS_CHANNEL, { percentages: { small: 3 } });

    expect(onSettingsChanged).toHaveBeenCalledWith({
      percentages: {
        small: 3,
        medium: 12,
        large: 26
      }
    });
  });
});

function createStore(settings: PointerMovementSettings = { percentages: { small: 4.5, medium: 12, large: 26 } }): JsonPointerMovementSettingsStore {
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
