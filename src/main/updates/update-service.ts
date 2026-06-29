import { autoUpdater as defaultAutoUpdater } from 'electron-updater';
import type { UpdateInfo as ElectronUpdateInfo, UpdateCheckResult } from 'electron-updater';
import type { UpdateDownloadProgress, UpdateInfo, UpdateInstallResult, UpdateState } from '../../shared/update';
import { createInitialUpdateState } from '../../shared/update';
import { appendUpdateInstallDiagnostic, type UpdateInstallDiagnosticEvent } from './update-install-diagnostics';
import { launchWindowsUpdateInstaller, type UpdateInstallerLaunchReason } from './update-installer-launcher';

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
  autoUpdater?: ElectronUpdaterAdapter;
  launchInstaller?: typeof launchWindowsUpdateInstaller;
  now?: () => Date;
  diagnosticsFilePath?: string | null;
  setInterval?: typeof setInterval;
  clearInterval?: typeof clearInterval;
  setTimeout?: typeof setTimeout;
  clearTimeout?: typeof clearTimeout;
  onStateChanged?: (state: UpdateState) => void;
};

type UpdateOperation = 'idle' | 'checking' | 'downloading';

export const UPDATE_POLL_INTERVAL_MS = 60 * 60 * 1000;
export const INITIAL_UPDATE_POLL_DELAY_MS = 30 * 1000;

export class UpdateService {
  private readonly isPackaged: boolean;
  private readonly platform: NodeJS.Platform;
  private readonly autoUpdater: ElectronUpdaterAdapter;
  private readonly launchInstaller: typeof launchWindowsUpdateInstaller;
  private readonly now: () => Date;
  private readonly diagnosticsFilePath: string | null;
  private readonly setPollInterval: typeof setInterval;
  private readonly clearPollInterval: typeof clearInterval;
  private readonly setPollTimeout: typeof setTimeout;
  private readonly clearPollTimeout: typeof clearTimeout;
  private readonly notifyStateChanged: (state: UpdateState) => void;
  private state: UpdateState;
  private operation: UpdateOperation = 'idle';
  private checkingPromise: Promise<UpdateState> | null = null;
  private downloadPromise: Promise<UpdateState> | null = null;
  private downloadedInstallerPath: string | null = null;
  private pollInterval: NodeJS.Timeout | null = null;
  private initialPollTimeout: NodeJS.Timeout | null = null;

  constructor(options: UpdateServiceOptions) {
    this.isPackaged = options.isPackaged;
    this.platform = options.platform;
    this.autoUpdater = options.autoUpdater ?? defaultAutoUpdater;
    this.launchInstaller = options.launchInstaller ?? launchWindowsUpdateInstaller;
    this.now = options.now ?? (() => new Date());
    this.diagnosticsFilePath = options.diagnosticsFilePath ?? null;
    this.setPollInterval = options.setInterval ?? setInterval;
    this.clearPollInterval = options.clearInterval ?? clearInterval;
    this.setPollTimeout = options.setTimeout ?? setTimeout;
    this.clearPollTimeout = options.clearTimeout ?? clearTimeout;
    this.notifyStateChanged = options.onStateChanged ?? (() => undefined);
    this.state = createInitialUpdateState(options.currentVersion);

    this.autoUpdater.autoDownload = false;
    this.autoUpdater.autoInstallOnAppQuit = false;
    this.registerUpdaterEvents();
  }

  getState(): UpdateState {
    return cloneState(this.state);
  }

  startAutomaticUpdateChecks(): void {
    if (this.unsupportedReason()) return;
    if (this.pollInterval || this.initialPollTimeout) return;

    this.initialPollTimeout = this.setPollTimeout(() => {
      this.initialPollTimeout = null;
      void this.runAutomaticUpdateCheck();
    }, INITIAL_UPDATE_POLL_DELAY_MS);

    this.pollInterval = this.setPollInterval(() => {
      void this.runAutomaticUpdateCheck();
    }, UPDATE_POLL_INTERVAL_MS);
  }

  stopAutomaticUpdateChecks(): void {
    if (this.initialPollTimeout) {
      this.clearPollTimeout(this.initialPollTimeout);
      this.initialPollTimeout = null;
    }

    if (this.pollInterval) {
      this.clearPollInterval(this.pollInterval);
      this.pollInterval = null;
    }
  }

  checkForUpdates(): Promise<UpdateState> {
    const unsupportedReason = this.unsupportedReason();
    if (unsupportedReason) {
      this.setState({
        ...this.state,
        info: {
          ...this.state.info,
          checkedAt: this.now().toISOString(),
          status: 'check_failed',
          reason: unsupportedReason
        },
        download: createIdleDownload()
      });
      return Promise.resolve(this.getState());
    }

    if (this.checkingPromise) return Promise.resolve(this.getState());

    this.operation = 'checking';
    this.downloadedInstallerPath = null;
    this.setState({
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
    });

    this.checkingPromise = this.autoUpdater
      .checkForUpdates()
      .then(() => this.getState())
      .catch((error) => {
        console.error('Switchify update check failed.', error);
        this.setState({
          ...this.state,
          info: {
            ...this.state.info,
            checkedAt: this.now().toISOString(),
            status: 'check_failed',
            reason: 'network_error'
          }
        });
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
      this.setState({
        ...this.state,
        download: {
          ...createIdleDownload(),
          status: 'download_failed',
          reason: unsupportedReason
        }
      });
      return this.getState();
    }

    if (this.downloadPromise) return this.getState();

    if (this.state.info.status !== 'update_available') {
      this.setState({
        ...this.state,
        download: {
          ...createIdleDownload(),
          status: 'download_failed',
          reason: 'not_available'
        }
      });
      return this.getState();
    }

    this.operation = 'downloading';
    this.downloadedInstallerPath = null;
    this.setState({
      ...this.state,
      download: {
        status: 'downloading',
        downloadedBytes: 0,
        totalBytes: null,
        percent: null
      }
    });

    this.downloadPromise = this.autoUpdater
      .downloadUpdate()
      .then((downloadedPaths) => {
        this.downloadedInstallerPath = firstInstallerPath(downloadedPaths);
        return this.getState();
      })
      .catch((error) => {
        console.error('Switchify update download failed.', error);
        this.downloadedInstallerPath = null;
        this.setState({
          ...this.state,
          download: {
            ...this.state.download,
            status: 'download_failed',
            reason: 'network_error'
          }
        });
        return this.getState();
      })
      .finally(() => {
        this.operation = 'idle';
        this.downloadPromise = null;
      });

    return this.downloadPromise;
  }

  async installDownloadedUpdate(): Promise<UpdateInstallResult> {
    this.recordInstallDiagnostic('install_requested');
    const unsupportedReason = this.unsupportedReason();
    if (unsupportedReason) {
      return { ok: false, reason: unsupportedReason };
    }

    if (this.state.download.status !== 'downloaded') {
      return { ok: false, reason: 'not_downloaded' };
    }

    const result = await this.launchInstaller({ installerPath: this.downloadedInstallerPath });

    if (!result.ok) {
      console.error('Switchify update installer could not be opened.', result.reason);
      this.recordInstallDiagnostic(diagnosticEventForLaunchFailure(result.reason), result.reason);
      return { ok: false, reason: result.reason };
    }

    this.recordInstallDiagnostic('installer_started');
    return { ok: true };
  }

  recordInstallCancelled(): void {
    this.recordInstallDiagnostic('confirmation_cancelled');
  }

  private registerUpdaterEvents(): void {
    this.autoUpdater.on('update-available', (rawInfo) => {
      const info = rawInfo as ElectronUpdateInfo;
      this.downloadedInstallerPath = null;
      this.setState({
        info: updateInfo(this.state.info, info, {
          checkedAt: this.now().toISOString(),
          status: 'update_available',
          reason: undefined
        }),
        download: createIdleDownload()
      });
    });

    this.autoUpdater.on('update-not-available', (rawInfo) => {
      const info = rawInfo as ElectronUpdateInfo;
      this.downloadedInstallerPath = null;
      this.setState({
        ...this.state,
        info: updateInfo(this.state.info, info, {
          checkedAt: this.now().toISOString(),
          latestVersion: info.version || this.state.info.currentVersion,
          status: 'up_to_date',
          reason: undefined
        }),
        download: createIdleDownload()
      });
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

      this.setState({
        ...this.state,
        download: {
          status: 'downloading',
          downloadedBytes,
          totalBytes,
          percent
        }
      });
    });

    this.autoUpdater.on('update-downloaded', (rawInfo) => {
      const info = rawInfo as ElectronUpdateInfo;
      this.setState({
        ...this.state,
        info: updateInfo(this.state.info, info),
        download: {
          status: 'downloaded',
          downloadedBytes: this.state.download.downloadedBytes,
          totalBytes: this.state.download.totalBytes,
          percent: 100
        }
      });
    });

    this.autoUpdater.on('error', (error) => {
      console.error('Switchify updater error.', error);
      if (this.operation === 'downloading') {
        this.downloadedInstallerPath = null;
        this.setState({
          ...this.state,
          download: {
            ...this.state.download,
            status: 'download_failed',
            reason: 'network_error'
          }
        });
        return;
      }

      this.setState({
        ...this.state,
        info: {
          ...this.state.info,
          checkedAt: this.now().toISOString(),
          status: 'check_failed',
          reason: 'network_error'
        }
      });
    });
  }

  private async runAutomaticUpdateCheck(): Promise<void> {
    if (this.operation !== 'idle') return;
    if (this.state.download.status === 'downloading' || this.state.download.status === 'downloaded') return;

    await this.checkForUpdates();
  }

  private setState(state: UpdateState): void {
    this.state = state;
    this.notifyStateChanged(this.getState());
  }

  private unsupportedReason(): 'not_packaged' | 'not_supported' | null {
    if (!this.isPackaged) return 'not_packaged';
    if (this.platform !== 'win32') return 'not_supported';
    return null;
  }

  private recordInstallDiagnostic(event: UpdateInstallDiagnosticEvent, reason?: string): void {
    if (!this.diagnosticsFilePath) return;

    appendUpdateInstallDiagnostic(this.diagnosticsFilePath, {
      event,
      at: this.now().toISOString(),
      version: this.state.info.currentVersion,
      ...(reason ? { reason } : {})
    });
  }

}

function firstInstallerPath(paths: string[]): string | null {
  return paths.find((path) => path.toLowerCase().endsWith('.exe')) ?? paths[0] ?? null;
}

function diagnosticEventForLaunchFailure(reason: UpdateInstallerLaunchReason): UpdateInstallDiagnosticEvent {
  switch (reason) {
    case 'installer_unavailable':
      return 'installer_missing';
    case 'installer_launch_failed':
      return 'installer_launch_failed';
  }
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
