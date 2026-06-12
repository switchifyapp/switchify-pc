import { createWriteStream, existsSync } from 'node:fs';
import { mkdir, rename, unlink } from 'node:fs/promises';
import { basename, join } from 'node:path';
import { once } from 'node:events';
import type { UpdateDownloadProgress, UpdateInfo, UpdateState } from '../../shared/update';
import { createInitialUpdateState } from '../../shared/update';
import { compareVersions, normalizeVersion } from './version';

const LATEST_RELEASE_URL = 'https://api.github.com/repos/switchifyapp/switchify-pc/releases/latest';

type GitHubReleaseAsset = {
  name: string;
  browser_download_url: string;
};

type GitHubRelease = {
  tag_name: string;
  name: string | null;
  html_url: string;
  draft: boolean;
  prerelease: boolean;
  assets: GitHubReleaseAsset[];
};

type ReleaseFetchResult = {
  status: number;
  release: unknown;
};

type InstallerAsset = {
  name: string;
  url: string;
};

type DownloadProgress = {
  downloadedBytes: number;
  totalBytes: number | null;
};

export type UpdateServiceOptions = {
  currentVersion: string;
  downloadsPath: string;
  fetchLatestRelease?: () => Promise<ReleaseFetchResult>;
  downloadAsset?: (
    asset: InstallerAsset,
    destinationPath: string,
    onProgress: (progress: DownloadProgress) => void
  ) => Promise<void>;
  showItemInFolder?: (filePath: string) => void;
  now?: () => Date;
};

export class UpdateService {
  private readonly downloadsPath: string;
  private readonly fetchLatestRelease: () => Promise<ReleaseFetchResult>;
  private readonly downloadAsset: (
    asset: InstallerAsset,
    destinationPath: string,
    onProgress: (progress: DownloadProgress) => void
  ) => Promise<void>;
  private readonly showItemInFolder: (filePath: string) => void;
  private readonly now: () => Date;
  private state: UpdateState;
  private latestAsset: InstallerAsset | null = null;
  private checkingPromise: Promise<UpdateState> | null = null;
  private downloadPromise: Promise<UpdateState> | null = null;

  constructor(options: UpdateServiceOptions) {
    this.downloadsPath = options.downloadsPath;
    this.fetchLatestRelease = options.fetchLatestRelease ?? fetchLatestGitHubRelease;
    this.downloadAsset = options.downloadAsset ?? downloadAssetToFile;
    this.showItemInFolder = options.showItemInFolder ?? (() => {});
    this.now = options.now ?? (() => new Date());
    this.state = createInitialUpdateState(options.currentVersion);
  }

  getState(): UpdateState {
    return cloneState(this.state);
  }

  checkForUpdates(): Promise<UpdateState> {
    if (this.checkingPromise) return Promise.resolve(this.getState());

    this.latestAsset = null;
    this.state = {
      info: {
        ...this.state.info,
        latestVersion: null,
        releaseName: null,
        releaseUrl: null,
        installerAssetName: null,
        status: 'checking',
        reason: undefined
      },
      download: createIdleDownload()
    };

    this.checkingPromise = this.runCheck()
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
        this.checkingPromise = null;
      });

    return this.checkingPromise;
  }

  async downloadUpdate(): Promise<UpdateState> {
    if (this.downloadPromise) return this.getState();

    const asset = this.latestAsset;
    if (!asset || this.state.info.status !== 'update_available') {
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

    this.state = {
      ...this.state,
      download: {
        status: 'downloading',
        downloadedBytes: 0,
        totalBytes: null,
        percent: null,
        filePath: null
      }
    };

    this.downloadPromise = this.runDownload(asset).finally(() => {
      this.downloadPromise = null;
    });
    return this.downloadPromise;
  }

  showDownloadedUpdate(): { ok: boolean; reason?: string } {
    const filePath = this.state.download.filePath;
    if (!filePath || this.state.download.status !== 'downloaded') {
      return { ok: false, reason: 'not_downloaded' };
    }

    this.showItemInFolder(filePath);
    return { ok: true };
  }

  private async runCheck(): Promise<UpdateState> {
    const result = await this.fetchLatestRelease();

    if (result.status === 404) {
      this.state = {
        ...this.state,
        info: {
          ...this.state.info,
          checkedAt: this.now().toISOString(),
          status: 'no_release',
          reason: 'no_release'
        }
      };
      return this.getState();
    }

    if (result.status !== 200 || !isGitHubRelease(result.release)) {
      this.state = {
        ...this.state,
        info: {
          ...this.state.info,
          checkedAt: this.now().toISOString(),
          status: 'check_failed',
          reason: result.status === 200 ? 'invalid_release' : 'network_error'
        }
      };
      return this.getState();
    }

    const release = result.release;
    if (release.draft || release.prerelease) {
      this.state = {
        ...this.state,
        info: {
          ...this.state.info,
          checkedAt: this.now().toISOString(),
          status: 'check_failed',
          reason: 'invalid_release'
        }
      };
      return this.getState();
    }

    const latestVersion = normalizeVersion(release.tag_name);
    const comparison = compareVersions(release.tag_name, this.state.info.currentVersion);
    if (!latestVersion || comparison === null) {
      this.state = {
        ...this.state,
        info: releaseInfo(this.state.info, release, latestVersion, {
          checkedAt: this.now().toISOString(),
          status: 'check_failed',
          reason: 'invalid_version'
        })
      };
      return this.getState();
    }

    if (comparison <= 0) {
      this.state = {
        ...this.state,
        info: releaseInfo(this.state.info, release, latestVersion, {
          checkedAt: this.now().toISOString(),
          status: 'up_to_date',
          reason: undefined
        })
      };
      return this.getState();
    }

    const asset = selectInstallerAsset(release.assets, latestVersion);
    if (!asset) {
      this.state = {
        ...this.state,
        info: releaseInfo(this.state.info, release, latestVersion, {
          checkedAt: this.now().toISOString(),
          status: 'check_failed',
          reason: 'installer_missing'
        })
      };
      return this.getState();
    }

    this.latestAsset = {
      name: asset.name,
      url: asset.browser_download_url
    };
    this.state = {
      ...this.state,
      info: releaseInfo(this.state.info, release, latestVersion, {
        checkedAt: this.now().toISOString(),
        installerAssetName: asset.name,
        status: 'update_available',
        reason: undefined
      })
    };
    return this.getState();
  }

  private async runDownload(asset: InstallerAsset): Promise<UpdateState> {
    await mkdir(this.downloadsPath, { recursive: true });
    const destinationPath = resolveAvailableDownloadPath(this.downloadsPath, asset.name);
    const partialPath = `${destinationPath}.download`;

    try {
      if (existsSync(partialPath)) {
        await unlink(partialPath);
      }

      await this.downloadAsset(asset, partialPath, (progress) => {
        this.state = {
          ...this.state,
          download: {
            status: 'downloading',
            downloadedBytes: progress.downloadedBytes,
            totalBytes: progress.totalBytes,
            percent:
              progress.totalBytes && progress.totalBytes > 0
                ? Math.min(100, Math.round((progress.downloadedBytes / progress.totalBytes) * 100))
                : null,
            filePath: null
          }
        };
      });
      await rename(partialPath, destinationPath);
      this.state = {
        ...this.state,
        download: {
          status: 'downloaded',
          downloadedBytes: this.state.download.downloadedBytes,
          totalBytes: this.state.download.totalBytes,
          percent: this.state.download.totalBytes ? 100 : this.state.download.percent,
          filePath: destinationPath
        }
      };
    } catch (error) {
      console.error('Switchify update download failed.', error);
      await unlinkIfExists(partialPath);
      this.state = {
        ...this.state,
        download: {
          status: 'download_failed',
          downloadedBytes: this.state.download.downloadedBytes,
          totalBytes: this.state.download.totalBytes,
          percent: this.state.download.percent,
          filePath: null,
          reason: toDownloadFailureReason(error)
        }
      };
    }

    return this.getState();
  }
}

export function selectInstallerAsset(
  assets: GitHubReleaseAsset[],
  version: string
): GitHubReleaseAsset | null {
  const preferredName = `Switchify-PC-Setup-${version}-x64.exe`.toLowerCase();
  const preferred = assets.find((asset) => asset.name.toLowerCase() === preferredName);
  if (preferred) return preferred;

  return (
    assets.find((asset) => {
      const name = asset.name.toLowerCase();
      return (
        name.endsWith('.exe') &&
        name.includes('setup') &&
        name.includes('x64') &&
        !name.includes('blockmap') &&
        !name.includes('sha') &&
        !name.includes('latest')
      );
    }) ?? null
  );
}

function releaseInfo(
  previous: UpdateInfo,
  release: GitHubRelease,
  latestVersion: string | null,
  patch: Partial<UpdateInfo>
): UpdateInfo {
  return {
    ...previous,
    latestVersion,
    releaseName: release.name,
    releaseUrl: release.html_url,
    installerAssetName: null,
    ...patch
  };
}

function createIdleDownload(): UpdateDownloadProgress {
  return {
    status: 'idle',
    downloadedBytes: 0,
    totalBytes: null,
    percent: null,
    filePath: null
  };
}

async function fetchLatestGitHubRelease(): Promise<ReleaseFetchResult> {
  const response = await fetch(LATEST_RELEASE_URL, {
    headers: {
      Accept: 'application/vnd.github+json',
      'User-Agent': 'Switchify-PC'
    }
  });

  if (response.status === 404) {
    return { status: 404, release: null };
  }

  if (!response.ok) {
    return { status: response.status, release: null };
  }

  try {
    return { status: response.status, release: await response.json() };
  } catch {
    return { status: response.status, release: null };
  }
}

async function downloadAssetToFile(
  asset: InstallerAsset,
  destinationPath: string,
  onProgress: (progress: DownloadProgress) => void
): Promise<void> {
  const response = await fetch(asset.url, {
    headers: {
      'User-Agent': 'Switchify-PC'
    }
  });

  if (!response.ok || !response.body) {
    throw createDownloadFailure('network_error');
  }

  const totalBytesHeader = response.headers.get('content-length');
  const totalBytes = totalBytesHeader ? Number(totalBytesHeader) : null;
  const file = createWriteStream(destinationPath, { flags: 'wx' });
  const reader = response.body.getReader();
  let downloadedBytes = 0;

  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      downloadedBytes += value.byteLength;
      if (!file.write(value)) {
        await once(file, 'drain');
      }
      onProgress({
        downloadedBytes,
        totalBytes: Number.isFinite(totalBytes) ? totalBytes : null
      });
    }
  } catch (error) {
    file.destroy();
    throw error;
  }

  file.end();
  await once(file, 'finish');
}

function isGitHubRelease(value: unknown): value is GitHubRelease {
  if (!value || typeof value !== 'object') return false;
  const release = value as Partial<GitHubRelease>;
  return (
    typeof release.tag_name === 'string' &&
    (typeof release.name === 'string' || release.name === null) &&
    typeof release.html_url === 'string' &&
    typeof release.draft === 'boolean' &&
    typeof release.prerelease === 'boolean' &&
    Array.isArray(release.assets) &&
    release.assets.every(
      (asset) =>
        asset &&
        typeof asset === 'object' &&
        typeof (asset as Partial<GitHubReleaseAsset>).name === 'string' &&
        typeof (asset as Partial<GitHubReleaseAsset>).browser_download_url === 'string'
    )
  );
}

function resolveAvailableDownloadPath(downloadsPath: string, assetName: string): string {
  const sanitizedName = sanitizeFilename(assetName);
  const extensionIndex = sanitizedName.lastIndexOf('.');
  const stem = extensionIndex > 0 ? sanitizedName.slice(0, extensionIndex) : sanitizedName;
  const extension = extensionIndex > 0 ? sanitizedName.slice(extensionIndex) : '';
  let candidate = join(downloadsPath, sanitizedName);
  let suffix = 1;

  while (existsSync(candidate) || existsSync(`${candidate}.download`)) {
    candidate = join(downloadsPath, `${stem} (${suffix})${extension}`);
    suffix += 1;
  }

  return candidate;
}

function sanitizeFilename(assetName: string): string {
  const name = basename(assetName).replace(/[<>:"/\\|?*\u0000-\u001f]/g, '_').trim();
  return name.length > 0 ? name : 'Switchify-PC-Setup.exe';
}

function createDownloadFailure(reason: UpdateDownloadProgress['reason']): Error & { reason: UpdateDownloadProgress['reason'] } {
  const error = new Error(reason);
  return Object.assign(error, { reason });
}

function toDownloadFailureReason(error: unknown): UpdateDownloadProgress['reason'] {
  if (error && typeof error === 'object' && 'reason' in error) {
    const reason = (error as { reason?: unknown }).reason;
    if (reason === 'network_error' || reason === 'filesystem_error' || reason === 'installer_missing' || reason === 'not_available') {
      return reason;
    }
  }
  return 'network_error';
}

async function unlinkIfExists(filePath: string): Promise<void> {
  try {
    await unlink(filePath);
  } catch {
    // Ignore cleanup failures for partial downloads.
  }
}

function cloneState(state: UpdateState): UpdateState {
  return {
    info: { ...state.info },
    download: { ...state.download }
  };
}
