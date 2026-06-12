import { describe, expect, it } from 'vitest';
import type { UpdateDownloadProgress, UpdateInfo } from '../shared/update';
import { updateCheckMessage, updateDownloadMessage } from './updates';

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
});

describe('updateDownloadMessage', () => {
  it('does not show the full local path after download', () => {
    expect(
      updateDownloadMessage(
        download({
          status: 'downloaded',
          filePath: 'C:\\Users\\oamcg\\Downloads\\Switchify-PC-Setup-0.1.1-x64.exe'
        })
      )
    ).toBe('Downloaded to Downloads.');
  });

  it('formats download progress when a percent is available', () => {
    expect(updateDownloadMessage(download({ status: 'downloading', percent: 42 }))).toBe('Downloading 42%.');
  });

  it('uses generic download text without a percent', () => {
    expect(updateDownloadMessage(download({ status: 'downloading', percent: null }))).toBe('Downloading...');
  });
});

function info(overrides: Partial<UpdateInfo>): UpdateInfo {
  return {
    currentVersion: '0.1.0',
    latestVersion: null,
    releaseName: null,
    releaseUrl: null,
    installerAssetName: null,
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
    filePath: null,
    ...overrides
  };
}
