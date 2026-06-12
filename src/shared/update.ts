export type UpdateCheckStatus =
  | 'not_checked'
  | 'checking'
  | 'up_to_date'
  | 'update_available'
  | 'no_release'
  | 'check_failed';

export type UpdateDownloadStatus =
  | 'idle'
  | 'downloading'
  | 'downloaded'
  | 'download_failed';

export type UpdateInfo = {
  currentVersion: string;
  latestVersion: string | null;
  releaseName: string | null;
  releaseUrl: string | null;
  installerAssetName: string | null;
  checkedAt: string | null;
  status: UpdateCheckStatus;
  reason?: 'no_release' | 'network_error' | 'invalid_release' | 'installer_missing' | 'invalid_version';
};

export type UpdateDownloadProgress = {
  status: UpdateDownloadStatus;
  downloadedBytes: number;
  totalBytes: number | null;
  percent: number | null;
  filePath: string | null;
  reason?: 'network_error' | 'filesystem_error' | 'installer_missing' | 'not_available';
};

export type UpdateState = {
  info: UpdateInfo;
  download: UpdateDownloadProgress;
};

export function createInitialUpdateState(currentVersion: string): UpdateState {
  return {
    info: {
      currentVersion,
      latestVersion: null,
      releaseName: null,
      releaseUrl: null,
      installerAssetName: null,
      checkedAt: null,
      status: 'not_checked'
    },
    download: {
      status: 'idle',
      downloadedBytes: 0,
      totalBytes: null,
      percent: null,
      filePath: null
    }
  };
}
