import { beforeEach, describe, expect, it, vi } from 'vitest';
import { INSTALL_DOWNLOADED_UPDATE_CHANNEL } from '../../shared/ipc-channels';
import {
  isInstallUpdateConfirmed,
  registerUpdateIpc,
  UPDATE_INSTALL_CONFIRMATION_OPTIONS,
  type UpdateInstallConfirmation
} from './update-ipc';
import type { UpdateService } from './update-service';
import type { UpdateInstallResult } from '../../shared/update';

type IpcHandler = (event: Electron.IpcMainInvokeEvent, ...args: unknown[]) => unknown;

const ipcHandlers = new Map<string, IpcHandler>();

vi.mock('electron', () => ({
  BrowserWindow: {
    fromWebContents: vi.fn()
  },
  dialog: {
    showMessageBox: vi.fn()
  },
  ipcMain: {
    handle: vi.fn((channel: string, handler: IpcHandler) => {
      ipcHandlers.set(channel, handler);
    })
  }
}));

describe('registerUpdateIpc', () => {
  beforeEach(() => {
    ipcHandlers.clear();
  });

  it('installs a downloaded update after confirmation', async () => {
    const updateService = createUpdateService({ downloaded: true, installResult: { ok: true } });
    const confirmInstallDownloadedUpdate = vi.fn<UpdateInstallConfirmation>(async () => true);

    registerUpdateIpc(updateService, { confirmInstallDownloadedUpdate });

    await expect(invokeInstall()).resolves.toEqual({ ok: true });
    expect(confirmInstallDownloadedUpdate).toHaveBeenCalledTimes(1);
    expect(updateService.installDownloadedUpdate).toHaveBeenCalledTimes(1);
  });

  it('returns installer launch failures after confirmation', async () => {
    const updateService = createUpdateService({
      downloaded: true,
      installResult: { ok: false, reason: 'installer_launch_failed' }
    });
    const confirmInstallDownloadedUpdate = vi.fn<UpdateInstallConfirmation>(async () => true);

    registerUpdateIpc(updateService, { confirmInstallDownloadedUpdate });

    await expect(invokeInstall()).resolves.toEqual({ ok: false, reason: 'installer_launch_failed' });
    expect(confirmInstallDownloadedUpdate).toHaveBeenCalledTimes(1);
    expect(updateService.installDownloadedUpdate).toHaveBeenCalledTimes(1);
  });

  it('does not install a downloaded update when confirmation is cancelled', async () => {
    const updateService = createUpdateService({ downloaded: true, installResult: { ok: true } });
    const confirmInstallDownloadedUpdate = vi.fn<UpdateInstallConfirmation>(async () => false);

    registerUpdateIpc(updateService, { confirmInstallDownloadedUpdate });

    await expect(invokeInstall()).resolves.toEqual({ ok: false, reason: 'cancelled' });
    expect(confirmInstallDownloadedUpdate).toHaveBeenCalledTimes(1);
    expect(updateService.installDownloadedUpdate).not.toHaveBeenCalled();
    expect(updateService.recordInstallCancelled).toHaveBeenCalledTimes(1);
  });

  it('delegates to the update service without confirmation when no update is downloaded', async () => {
    const updateService = createUpdateService({
      downloaded: false,
      installResult: { ok: false, reason: 'not_downloaded' }
    });
    const confirmInstallDownloadedUpdate = vi.fn<UpdateInstallConfirmation>(async () => true);

    registerUpdateIpc(updateService, { confirmInstallDownloadedUpdate });

    await expect(invokeInstall()).resolves.toEqual({ ok: false, reason: 'not_downloaded' });
    expect(confirmInstallDownloadedUpdate).not.toHaveBeenCalled();
    expect(updateService.installDownloadedUpdate).toHaveBeenCalledTimes(1);
  });
});

describe('update install confirmation options', () => {
  it('warns that the installer may ask the user to close the app', () => {
    expect(UPDATE_INSTALL_CONFIRMATION_OPTIONS.type).toBe('warning');
    expect(UPDATE_INSTALL_CONFIRMATION_OPTIONS.message).toContain('Open the downloaded Switchify PC installer');
    expect(UPDATE_INSTALL_CONFIRMATION_OPTIONS.detail).toContain('installer will open');
    expect(UPDATE_INSTALL_CONFIRMATION_OPTIONS.detail).toContain('may ask you to close Switchify PC');
    expect(UPDATE_INSTALL_CONFIRMATION_OPTIONS.buttons).toEqual(['Open installer', 'Cancel']);
    expect(UPDATE_INSTALL_CONFIRMATION_OPTIONS.defaultId).toBe(1);
    expect(UPDATE_INSTALL_CONFIRMATION_OPTIONS.cancelId).toBe(1);
  });

  it('treats only the install button as confirmation', () => {
    expect(isInstallUpdateConfirmed(0)).toBe(true);
    expect(isInstallUpdateConfirmed(1)).toBe(false);
  });
});

function createUpdateService({
  downloaded,
  installResult
}: {
  downloaded: boolean;
  installResult: UpdateInstallResult;
}): UpdateService {
  return {
    getState: vi.fn(() => ({
      info: {
        currentVersion: '0.1.10',
        latestVersion: null,
        releaseName: null,
        releaseNotes: null,
        checkedAt: null,
        status: 'not_checked'
      },
      download: {
        status: downloaded ? 'downloaded' : 'idle',
        downloadedBytes: 0,
        totalBytes: null,
        percent: downloaded ? 100 : null
      }
    })),
    checkForUpdates: vi.fn(),
    downloadUpdate: vi.fn(),
    installDownloadedUpdate: vi.fn(async () => installResult),
    recordInstallCancelled: vi.fn()
  } as unknown as UpdateService;
}

function invokeInstall(): Promise<unknown> {
  const handler = ipcHandlers.get(INSTALL_DOWNLOADED_UPDATE_CHANNEL);
  if (!handler) throw new Error('Install handler was not registered.');
  return Promise.resolve(handler({ sender: {} } as Electron.IpcMainInvokeEvent));
}
