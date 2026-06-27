import { describe, expect, it } from 'vitest';
import type { UpdateDownloadProgress, UpdateInfo, UpdateState } from '../shared/update';
import {
  canDownloadUpdate,
  updateCheckMessage,
  updateDownloadMessage,
  updateIndicatorState,
  updateInstallMessage
} from './updates';

describe('updateCheckMessage', () => {
  it('formats not checked text as a terminal sentence', () => {
    expect(updateCheckMessage(info({ status: 'not_checked' }))).toBe('Not checked yet.');
  });

  it('formats checking text with three periods', () => {
    expect(updateCheckMessage(info({ status: 'checking' }))).toBe('Checking...');
  });

  it('includes the latest version when an update is available', () => {
    expect(updateCheckMessage(info({ status: 'update_available', latestVersion: '0.1.1' }))).toBe(
      'Update available: v0.1.1.'
    );
  });

  it('uses non-sensitive text for failed checks', () => {
    expect(updateCheckMessage(info({ status: 'check_failed', reason: 'network_error' }))).toBe(
      'Could not check for updates.'
    );
  });

  it('explains unsupported unpackaged builds', () => {
    expect(updateCheckMessage(info({ status: 'check_failed', reason: 'not_packaged' }))).toBe(
      'Updates are only available in packaged builds.'
    );
  });

  it('explains unsupported platforms', () => {
    expect(updateCheckMessage(info({ status: 'check_failed', reason: 'not_supported' }))).toBe(
      'Updates are not available on this platform.'
    );
  });
});

describe('updateDownloadMessage', () => {
  it('formats downloaded updates as ready to install', () => {
    expect(updateDownloadMessage(download({ status: 'downloaded' }))).toBe('Update downloaded and ready to install.');
  });

  it('formats download progress when a percent is available', () => {
    expect(updateDownloadMessage(download({ status: 'downloading', percent: 42 }))).toBe('Downloading 42%.');
  });

  it('uses generic download text without a percent', () => {
    expect(updateDownloadMessage(download({ status: 'downloading', percent: null }))).toBe('Downloading...');
  });
});

describe('canDownloadUpdate', () => {
  it('allows download when an update is available and download is idle', () => {
    expect(
      canDownloadUpdate(
        state({
          info: info({ status: 'update_available' }),
          download: download({ status: 'idle' })
        })
      )
    ).toBe(true);
  });

  it('allows retry when an update is available and download failed', () => {
    expect(
      canDownloadUpdate(
        state({
          info: info({ status: 'update_available' }),
          download: download({ status: 'download_failed' })
        })
      )
    ).toBe(true);
  });

  it('does not allow download while downloading', () => {
    expect(
      canDownloadUpdate(
        state({
          info: info({ status: 'update_available' }),
          download: download({ status: 'downloading' })
        })
      )
    ).toBe(false);
  });

  it('does not allow download after update is downloaded', () => {
    expect(
      canDownloadUpdate(
        state({
          info: info({ status: 'update_available' }),
          download: download({ status: 'downloaded' })
        })
      )
    ).toBe(false);
  });

  it('does not allow download when no update is available', () => {
    expect(
      canDownloadUpdate(
        state({
          info: info({ status: 'up_to_date' }),
          download: download({ status: 'idle' })
        })
      )
    ).toBe(false);
  });
});

describe('updateInstallMessage', () => {
  it('does not show a message for cancel or no error', () => {
    expect(updateInstallMessage(null)).toBeNull();
    expect(updateInstallMessage('cancelled')).toBeNull();
  });

  it('explains when the update is not downloaded', () => {
    expect(updateInstallMessage('not_downloaded')).toBe('The update is not downloaded yet.');
  });

  it('explains unsupported install environments', () => {
    expect(updateInstallMessage('not_packaged')).toBe('Updates are only available in the installed app.');
    expect(updateInstallMessage('not_supported')).toBe('Updates are only supported on Windows.');
  });

  it('explains missing installer files', () => {
    expect(updateInstallMessage('installer_unavailable')).toBe(
      'The downloaded installer could not be found. Download the update again.'
    );
  });

  it('explains missing elevation support', () => {
    expect(updateInstallMessage('elevation_helper_unavailable')).toBe(
      'The update installer could not request permission to install. Reinstall Switchify PC from the latest installer.'
    );
  });

  it('explains installer launch failures', () => {
    expect(updateInstallMessage('installer_launch_failed')).toBe(
      'The update installer could not be started. Download the update again or run the installer manually.'
    );
  });
});

describe('updateIndicatorState', () => {
  it('hides the indicator without update state', () => {
    expect(updateIndicatorState(null)).toBe('hidden');
  });

  it('hides the indicator before updates are checked', () => {
    expect(updateIndicatorState(state({ info: info({ status: 'not_checked' }) }))).toBe('hidden');
  });

  it('hides the indicator when the app is up to date', () => {
    expect(updateIndicatorState(state({ info: info({ status: 'up_to_date' }) }))).toBe('hidden');
  });

  it('hides the indicator when update checks fail', () => {
    expect(updateIndicatorState(state({ info: info({ status: 'check_failed' }) }))).toBe('hidden');
  });

  it('shows an available indicator when an update is available and idle', () => {
    expect(
      updateIndicatorState(
        state({
          info: info({ status: 'update_available' }),
          download: download({ status: 'idle' })
        })
      )
    ).toBe('available');
  });

  it('shows an available indicator when an update download failed', () => {
    expect(
      updateIndicatorState(
        state({
          info: info({ status: 'update_available' }),
          download: download({ status: 'download_failed' })
        })
      )
    ).toBe('available');
  });

  it('shows an available indicator while an update is downloading', () => {
    expect(
      updateIndicatorState(
        state({
          info: info({ status: 'update_available' }),
          download: download({ status: 'downloading' })
        })
      )
    ).toBe('available');
  });

  it('shows a downloaded indicator when an update is downloaded', () => {
    expect(
      updateIndicatorState(
        state({
          info: info({ status: 'up_to_date' }),
          download: download({ status: 'downloaded' })
        })
      )
    ).toBe('downloaded');
  });
});

function state(overrides: Partial<UpdateState>): UpdateState {
  return {
    info: info({}),
    download: download({}),
    ...overrides
  };
}

function info(overrides: Partial<UpdateInfo>): UpdateInfo {
  return {
    currentVersion: '0.1.0',
    latestVersion: null,
    releaseName: null,
    releaseNotes: null,
    checkedAt: null,
    status: 'not_checked',
    ...overrides
  };
}

function download(overrides: Partial<UpdateDownloadProgress>): UpdateDownloadProgress {
  return {
    status: 'idle',
    downloadedBytes: 0,
    totalBytes: null,
    percent: null,
    ...overrides
  };
}
