import { autoUpdater as defaultAutoUpdater } from 'electron-updater';
import type { UpdateInfo as ElectronUpdateInfo, UpdateCheckResult } from 'electron-updater';
import type { UpdateDownloadProgress, UpdateInfo, UpdateInstallResult, UpdateState } from '../../shared/update';
import { createInitialUpdateState } from '../../shared/update';
import { launchWindowsUpdateInstaller } from './update-installer-launcher';

type ElectronDownloadProgress = {
  transferred?: number;
  total?: number;
  percent?: number;
};

export type ElectronUpdaterAdapter = {
  autoDownload: boolean;
  autoInstallOnAppQuit: boolean;
  checkForUpdates(): Promise<UpdateCheckResult | null>;
  downloadUpdate(): Promise<Array<string>>;
  on(event: string, listener: (...args: unknown[]) => void): void;
};

export type UpdateServiceOptions = {
  currentVersion: string;
  isPackaged: boolean;
  platform: NodeJS.Platform;
  resourcesPath: string;
  autoUpdater?: ElectronUpdaterAdapter;
  launchInstaller?: typeof launchWindowsUpdateInstaller;
  quitApp?: () => void;
  now?: () => Date;
};

type UpdateOperation = 'idle' | 'checking' | 'downloading';

export class UpdateService {
  private readonly isPackaged: boolean;
  private readonly platform: NodeJS.Platform;
  private readonly resourcesPath: string;
  private readonly autoUpdater: ElectronUpdaterAdapter;
  private readonly launchInstaller: typeof launchWindowsUpdateInstaller;
  private readonly quitApp: () => void;
  private readonly now: () => Date;
  private state: UpdateState;
  private operation: UpdateOperation = 'idle';
  private checkingPromise: Promise<UpdateState> | null = null;
  private downloadPromise: Promise<UpdateState> | null = null;
  private downloadedInstallerPath: string | null = null;

  constructor(options: UpdateServiceOptions) {
    this.isPackaged = options.isPackaged;
    this.platform = options.platform;
    this.resourcesPath = options.resourcesPath;
    this.autoUpdater = options.autoUpdater ?? defaultAutoUpdater;
    this.launchInstaller = options.launchInstaller ?? launchWindowsUpdateInstaller;
    this.quitApp = options.quitApp ?? (() => undefined);
    this.now = options.now ?? (() => new Date());
    this.state = createInitialUpdateState(options.currentVersion);

    this.autoUpdater.autoDownload = false;
    this.autoUpdater.autoInstallOnAppQuit = false;
    this.registerUpdaterEvents();
  }

  getState(): UpdateState {
    return cloneState(this.state);
  }

  checkForUpdates(): Promise<UpdateState> {
    const unsupportedReason = this.unsupportedReason();
    if (unsupportedReason) {
      this.state = {
        ...this.state,
        info: {
          ...this.state.info,
          checkedAt: this.now().toISOString(),
          status: 'check_failed',
          reason: unsupportedReason
        },
        download: createIdleDownload()
      };
      return Promise.resolve(this.getState());
    }

    if (this.checkingPromise) return Promise.resolve(this.getState());

    this.operation = 'checking';
    this.downloadedInstallerPath = null;
    this.state = {
      info: {
        ...this.state.info,
        latestVersion: null,
        releaseName: null,
        releaseNotes: null,
        checkedAt: null,
        status: 'checking',
        reason: undefined
      },
      download: createIdleDownload()
    };

    this.checkingPromise = this.autoUpdater
      .checkForUpdates()
      .then(() => this.getState())
      .catch((error) => {
        console.error('Switchify update check failed.', error);
        this.state = {
          ...this.state,
          info: {
            ...this.state.info,
            checkedAt: this.now().toISOString(),
            status: 'check_failed',
            reason: 'network_error'
          }
        };
        return this.getState();
      })
      .finally(() => {
        this.operation = 'idle';
        this.checkingPromise = null;
      });

    return this.checkingPromise;
  }

  async downloadUpdate(): Promise<UpdateState> {
    const unsupportedReason = this.unsupportedReason();
    if (unsupportedReason) {
      this.state = {
        ...this.state,
        download: {
          ...createIdleDownload(),
          status: 'download_failed',
          reason: unsupportedReason
        }
      };
      return this.getState();
    }

    if (this.downloadPromise) return this.getState();

    if (this.state.info.status !== 'update_available') {
      this.state = {
        ...this.state,
        download: {
          ...createIdleDownload(),
          status: 'download_failed',
          reason: 'not_available'
        }
      };
      return this.getState();
    }

    this.operation = 'downloading';
    this.state = {
      ...this.state,
      download: {
        status: 'downloading',
        downloadedBytes: 0,
        totalBytes: null,
        percent: null
      }
    };

    this.downloadPromise = this.autoUpdater
      .downloadUpdate()
      .then((downloadedPaths) => {
        this.downloadedInstallerPath = firstInstallerPath(downloadedPaths);
        return this.getState();
      })
      .catch((error) => {
        console.error('Switchify update download failed.', error);
        this.downloadedInstallerPath = null;
        this.state = {
          ...this.state,
          download: {
            ...this.state.download,
            status: 'download_failed',
            reason: 'network_error'
          }
        };
        return this.getState();
      })
      .finally(() => {
        this.operation = 'idle';
        this.downloadPromise = null;
      });

    return this.downloadPromise;
  }

  async installDownloadedUpdate(): Promise<UpdateInstallResult> {
    const unsupportedReason = this.unsupportedReason();
    if (unsupportedReason) {
      return { ok: false, reason: unsupportedReason };
    }

    if (this.state.download.status !== 'downloaded') {
      return { ok: false, reason: 'not_downloaded' };
    }

    const result = await this.launchInstaller({
      installerPath: this.downloadedInstallerPath,
      resourcesPath: this.resourcesPath
    });

    if (!result.ok) {
      console.error('Switchify update installer could not be started.', result.reason);
      return { ok: false, reason: result.reason };
    }

    this.quitApp();
    return { ok: true };
  }

  private registerUpdaterEvents(): void {
    this.autoUpdater.on('update-available', (rawInfo) => {
      const info = rawInfo as ElectronUpdateInfo;
      this.downloadedInstallerPath = null;
      this.state = {
        info: updateInfo(this.state.info, info, {
          checkedAt: this.now().toISOString(),
          status: 'update_available',
          reason: undefined
        }),
        download: createIdleDownload()
      };
    });

    this.autoUpdater.on('update-not-available', (rawInfo) => {
      const info = rawInfo as ElectronUpdateInfo;
      this.state = {
        ...this.state,
        info: updateInfo(this.state.info, info, {
          checkedAt: this.now().toISOString(),
          latestVersion: info.version || this.state.info.currentVersion,
          status: 'up_to_date',
          reason: undefined
        }),
        download: createIdleDownload()
      };
    });

    this.autoUpdater.on('download-progress', (rawProgress) => {
      const progress = rawProgress as ElectronDownloadProgress;
      const downloadedBytes = Number.isFinite(progress.transferred) ? Number(progress.transferred) : 0;
      const totalBytes = Number.isFinite(progress.total) && Number(progress.total) > 0 ? Number(progress.total) : null;
      const percent =
        Number.isFinite(progress.percent) && Number(progress.percent) >= 0
          ? Math.min(100, Math.round(Number(progress.percent)))
          : totalBytes
            ? Math.min(100, Math.round((downloadedBytes / totalBytes) * 100))
            : null;

      this.state = {
        ...this.state,
        download: {
          status: 'downloading',
          downloadedBytes,
          totalBytes,
          percent
        }
      };
    });

    this.autoUpdater.on('update-downloaded', (rawInfo) => {
      const info = rawInfo as ElectronUpdateInfo;
      this.state = {
        ...this.state,
        info: updateInfo(this.state.info, info),
        download: {
          status: 'downloaded',
          downloadedBytes: this.state.download.downloadedBytes,
          totalBytes: this.state.download.totalBytes,
          percent: 100
        }
      };
    });

    this.autoUpdater.on('error', (error) => {
      console.error('Switchify updater error.', error);
      if (this.operation === 'downloading') {
        this.downloadedInstallerPath = null;
        this.state = {
          ...this.state,
          download: {
            ...this.state.download,
            status: 'download_failed',
            reason: 'network_error'
          }
        };
        return;
      }

      this.state = {
        ...this.state,
        info: {
          ...this.state.info,
          checkedAt: this.now().toISOString(),
          status: 'check_failed',
          reason: 'network_error'
        }
      };
    });
  }

  private unsupportedReason(): 'not_packaged' | 'not_supported' | null {
    if (!this.isPackaged) return 'not_packaged';
    if (this.platform !== 'win32') return 'not_supported';
    return null;
  }
}

function firstInstallerPath(paths: string[]): string | null {
  return paths.find((path) => path.toLowerCase().endsWith('.exe')) ?? paths[0] ?? null;
}

function updateInfo(
  previous: UpdateInfo,
  info: ElectronUpdateInfo,
  patch: Partial<UpdateInfo> = {}
): UpdateInfo {
  return {
    ...previous,
    latestVersion: info.version || previous.latestVersion,
    releaseName: toNullableString(info.releaseName),
    releaseNotes: normalizeReleaseNotes(info.releaseNotes),
    ...patch
  };
}

function normalizeReleaseNotes(notes: unknown): string | null {
  if (typeof notes === 'string') return notes;
  if (Array.isArray(notes)) {
    const text = notes
      .map((note) => {
        if (typeof note === 'string') return note;
        if (note && typeof note === 'object' && 'note' in note && typeof note.note === 'string') return note.note;
        return null;
      })
      .filter(Boolean)
      .join('\n\n');
    return text.length > 0 ? text : null;
  }
  return null;
}

function toNullableString(value: unknown): string | null {
  return typeof value === 'string' ? value : null;
}

function createIdleDownload(): UpdateDownloadProgress {
  return {
    status: 'idle',
    downloadedBytes: 0,
    totalBytes: null,
    percent: null
  };
}

function cloneState(state: UpdateState): UpdateState {
  return {
    info: { ...state.info },
    download: { ...state.download }
  };
}
