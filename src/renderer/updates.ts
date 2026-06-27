import type { UpdateDownloadProgress, UpdateInfo, UpdateInstallFailureReason, UpdateState } from '../shared/update';

export type UpdateIndicatorState = 'hidden' | 'available' | 'downloaded';

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

export function updateInstallMessage(reason: UpdateInstallFailureReason | null): string | null {
  switch (reason) {
    case null:
    case 'cancelled':
      return null;
    case 'not_downloaded':
      return 'The update is not downloaded yet.';
    case 'not_packaged':
      return 'Updates are only available in the installed app.';
    case 'not_supported':
      return 'Updates are only supported on Windows.';
    case 'installer_unavailable':
      return 'The downloaded installer could not be found. Download the update again.';
    case 'elevation_helper_unavailable':
      return 'The update installer could not request permission to install. Reinstall Switchify PC from the latest installer.';
    case 'installer_launch_failed':
      return 'The update installer could not be started. Download the update again or run the installer manually.';
  }
}

export function canDownloadUpdate(state: UpdateState | null): boolean {
  return (
    state?.info.status === 'update_available' &&
    (state.download.status === 'idle' || state.download.status === 'download_failed')
  );
}

export function updateIndicatorState(state: UpdateState | null): UpdateIndicatorState {
  if (!state) return 'hidden';
  if (state.download.status === 'downloaded') return 'downloaded';
  if (state.info.status === 'update_available') return 'available';
  return 'hidden';
}
