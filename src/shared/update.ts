export type UpdateCheckStatus =
  | 'not_checked'
  | 'checking'
  | 'up_to_date'
  | 'update_available'
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
  releaseNotes: string | null;
  checkedAt: string | null;
  status: UpdateCheckStatus;
  reason?: 'network_error' | 'invalid_update' | 'not_packaged' | 'not_supported';
};

export type UpdateDownloadProgress = {
  status: UpdateDownloadStatus;
  downloadedBytes: number;
  totalBytes: number | null;
  percent: number | null;
  reason?: 'network_error' | 'not_available' | 'not_packaged' | 'not_supported';
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
      releaseNotes: null,
      checkedAt: null,
      status: 'not_checked'
    },
    download: {
      status: 'idle',
      downloadedBytes: 0,
      totalBytes: null,
      percent: null
    }
  };
}
