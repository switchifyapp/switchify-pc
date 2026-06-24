import { ipcMain } from 'electron';
import { normalizePointerMovementSettings, type PointerMovementSettings } from '../shared/pointer-movement-settings';
import {
  GET_POINTER_MOVEMENT_SETTINGS_CHANNEL,
  SET_POINTER_MOVEMENT_SETTINGS_CHANNEL
} from '../shared/ipc-channels';
import type { JsonPointerMovementSettingsStore } from './pointer-movement-settings-store';

export function registerPointerMovementSettingsIpc(
  settingsStore: JsonPointerMovementSettingsStore,
  onSettingsChanged: (settings: PointerMovementSettings) => void
): void {
  ipcMain.handle(GET_POINTER_MOVEMENT_SETTINGS_CHANNEL, () => settingsStore.load());
  ipcMain.handle(SET_POINTER_MOVEMENT_SETTINGS_CHANNEL, (_event, settings: PointerMovementSettings) => {
    const normalized = settingsStore.save(normalizePointerMovementSettings(settings));
    onSettingsChanged(normalized);
    return normalized;
  });
}
