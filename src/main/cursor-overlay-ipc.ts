import { ipcMain } from 'electron';
import { normalizeCursorOverlaySettings, type CursorOverlaySettings } from '../shared/cursor-overlay-settings';
import {
  GET_CURSOR_OVERLAY_ENABLED_CHANNEL,
  GET_CURSOR_OVERLAY_SETTINGS_CHANNEL,
  SET_CURSOR_OVERLAY_SETTINGS_CHANNEL,
  SET_CURSOR_OVERLAY_ENABLED_CHANNEL
} from '../shared/ipc-channels';
import type { CursorOverlay } from './cursor-overlay';
import type { JsonCursorOverlaySettingsStore } from './cursor-overlay-settings-store';

export function registerCursorOverlayIpc(
  cursorOverlay: CursorOverlay,
  settingsStore: JsonCursorOverlaySettingsStore
): void {
  const saveSettings = (settings: CursorOverlaySettings): CursorOverlaySettings => {
    const normalized = settingsStore.save(settings);
    cursorOverlay.setSettings(normalized);
    return cursorOverlay.getSettings();
  };

  ipcMain.handle(GET_CURSOR_OVERLAY_SETTINGS_CHANNEL, () => cursorOverlay.getSettings());
  ipcMain.handle(SET_CURSOR_OVERLAY_SETTINGS_CHANNEL, (_event, settings: CursorOverlaySettings) =>
    saveSettings(normalizeCursorOverlaySettings(settings))
  );
  ipcMain.handle(GET_CURSOR_OVERLAY_ENABLED_CHANNEL, () => cursorOverlay.isEnabled());
  ipcMain.handle(SET_CURSOR_OVERLAY_ENABLED_CHANNEL, (_event, enabled: boolean) => {
    saveSettings({ ...cursorOverlay.getSettings(), enabled: Boolean(enabled) });
    return cursorOverlay.isEnabled();
  });
}
