import type { UpdateDownloadProgress, UpdateInfo, UpdateState } from '../shared/update';

export function updateCheckMessage(info: UpdateInfo | null): string {
  if (!info) return 'Loading...';

  if (info.reason === 'installer_missing') {
    return 'The release does not include a Windows installer.';
  }

  switch (info.status) {
    case 'not_checked':
      return 'Not checked yet.';
    case 'checking':
      return 'Checking...';
    case 'up_to_date':
      return 'Switchify PC is up to date.';
    case 'update_available':
      return `Update available: v${info.latestVersion ?? 'unknown'}.`;
    case 'no_release':
      return 'No public release found.';
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
    return 'Downloaded to Downloads.';
  }

  if (download.reason === 'installer_missing') {
    return 'The release does not include a Windows installer.';
  }

  return 'Could not download the update.';
}

export function canDownloadUpdate(state: UpdateState | null): boolean {
  return state?.info.status === 'update_available' && state.download.status !== 'downloading';
}
