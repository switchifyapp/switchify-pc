import type { UpdateDownloadProgress, UpdateInfo, UpdateState } from '../shared/update';

export function updateCheckMessage(info: UpdateInfo | null): string {
  if (!info) return 'Loading...';

  if (info.reason === 'not_packaged') return 'Updates are only available in packaged builds.';
  if (info.reason === 'not_supported') return 'Updates are not available on this platform.';

  switch (info.status) {
    case 'not_checked':
      return 'Not checked yet.';
    case 'checking':
      return 'Checking...';
    case 'up_to_date':
      return 'Switchify PC is up to date.';
    case 'update_available':
      return `Update available: v${info.latestVersion ?? 'unknown'}.`;
    case 'check_failed':
      return 'Could not check for updates.';
  }
}

export function updateDownloadMessage(download: UpdateDownloadProgress | null): string | null {
  if (!download || download.status === 'idle') return null;

  if (download.status === 'downloading') {
    return download.percent === null ? 'Downloading...' : `Downloading ${download.percent}%.`;
  }

  if (download.status === 'downloaded') {
    return 'Update downloaded and ready to install.';
  }

  if (download.reason === 'not_packaged') return 'Updates are only available in packaged builds.';
  if (download.reason === 'not_supported') return 'Updates are not available on this platform.';

  return 'Could not download the update.';
}

export function canDownloadUpdate(state: UpdateState | null): boolean {
  return state?.info.status === 'update_available' && state.download.status !== 'downloading';
}
