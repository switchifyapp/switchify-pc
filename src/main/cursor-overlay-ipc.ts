import { ipcMain } from 'electron';
import {
  GET_CURSOR_OVERLAY_ENABLED_CHANNEL,
  SET_CURSOR_OVERLAY_ENABLED_CHANNEL
} from '../shared/ipc-channels';
import type { CursorOverlay } from './cursor-overlay';

export function registerCursorOverlayIpc(cursorOverlay: CursorOverlay): void {
  ipcMain.handle(GET_CURSOR_OVERLAY_ENABLED_CHANNEL, () => cursorOverlay.isEnabled());
  ipcMain.handle(SET_CURSOR_OVERLAY_ENABLED_CHANNEL, (_event, enabled: boolean) => {
    cursorOverlay.setEnabled(Boolean(enabled));
    return cursorOverlay.isEnabled();
  });
}
