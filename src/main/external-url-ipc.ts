import { ipcMain, shell } from 'electron';
import { OPEN_EXTERNAL_URL_CHANNEL } from '../shared/ipc-channels';

export function registerExternalUrlIpc(): void {
  ipcMain.handle(OPEN_EXTERNAL_URL_CHANNEL, async (_event, rawUrl: unknown): Promise<{ ok: boolean; reason?: string }> => {
    if (typeof rawUrl !== 'string') return { ok: false, reason: 'invalid_url' };

    let url: URL;
    try {
      url = new URL(rawUrl);
    } catch {
      return { ok: false, reason: 'invalid_url' };
    }

    if (url.protocol !== 'https:' && url.protocol !== 'http:') {
      return { ok: false, reason: 'unsupported_protocol' };
    }

    try {
      await shell.openExternal(url.toString());
      return { ok: true };
    } catch {
      return { ok: false, reason: 'open_failed' };
    }
  });
}
