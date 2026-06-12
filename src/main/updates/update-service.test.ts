import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { afterEach, describe, expect, it } from 'vitest';
import { selectInstallerAsset, UpdateService } from './update-service';

const tempDirs: string[] = [];

afterEach(async () => {
  await Promise.all(tempDirs.map((dir) => rm(dir, { recursive: true, force: true })));
  tempDirs.length = 0;
});

describe('UpdateService.checkForUpdates', () => {
  it('maps a missing GitHub release to no_release', async () => {
    const service = serviceWithRelease({ status: 404, release: null });

    const state = await service.checkForUpdates();

    expect(state.info.status).toBe('no_release');
    expect(state.info.reason).toBe('no_release');
  });

  it('maps the same latest version to up_to_date', async () => {
    const service = serviceWithRelease({ status: 200, release: release({ tag_name: 'v0.1.0' }) });

    const state = await service.checkForUpdates();

    expect(state.info.status).toBe('up_to_date');
    expect(state.info.latestVersion).toBe('0.1.0');
  });

  it('maps a newer release with a matching setup asset to update_available', async () => {
    const service = serviceWithRelease({
      status: 200,
      release: release({
        tag_name: 'v0.1.1',
        assets: [asset('Switchify-PC-Setup-0.1.1-x64.exe')]
      })
    });

    const state = await service.checkForUpdates();

    expect(state.info.status).toBe('update_available');
    expect(state.info.installerAssetName).toBe('Switchify-PC-Setup-0.1.1-x64.exe');
  });

  it('reports installer_missing for a newer release without a Windows installer', async () => {
    const service = serviceWithRelease({
      status: 200,
      release: release({
        tag_name: 'v0.1.1',
        assets: [asset('Switchify-PC-Setup-0.1.1-arm64.exe')]
      })
    });

    const state = await service.checkForUpdates();

    expect(state.info.status).toBe('check_failed');
    expect(state.info.reason).toBe('installer_missing');
  });

  it('rejects draft or prerelease releases defensively', async () => {
    await expect(
      serviceWithRelease({ status: 200, release: release({ draft: true }) }).checkForUpdates()
    ).resolves.toMatchObject({ info: { status: 'check_failed', reason: 'invalid_release' } });
    await expect(
      serviceWithRelease({ status: 200, release: release({ prerelease: true }) }).checkForUpdates()
    ).resolves.toMatchObject({ info: { status: 'check_failed', reason: 'invalid_release' } });
  });

  it('maps network failures to check_failed', async () => {
    const service = new UpdateService({
      currentVersion: '0.1.0',
      downloadsPath: await tempDir(),
      fetchLatestRelease: async () => {
        throw new Error('network failed');
      },
      now: fixedNow
    });

    const state = await service.checkForUpdates();

    expect(state.info.status).toBe('check_failed');
    expect(state.info.reason).toBe('network_error');
  });
});

describe('UpdateService.downloadUpdate', () => {
  it('writes a completed installer file', async () => {
    const downloadsPath = await tempDir();
    const service = new UpdateService({
      currentVersion: '0.1.0',
      downloadsPath,
      fetchLatestRelease: async () => ({
        status: 200,
        release: release({ tag_name: 'v0.1.1', assets: [asset('Switchify-PC-Setup-0.1.1-x64.exe')] })
      }),
      downloadAsset: async (_asset, destinationPath, onProgress) => {
        await writeFile(destinationPath, 'installer');
        onProgress({ downloadedBytes: 9, totalBytes: 9 });
      },
      now: fixedNow
    });
    await service.checkForUpdates();

    const state = await service.downloadUpdate();

    expect(state.download.status).toBe('downloaded');
    expect(state.download.percent).toBe(100);
    expect(state.download.filePath).toBe(join(downloadsPath, 'Switchify-PC-Setup-0.1.1-x64.exe'));
    await expect(readFile(state.download.filePath ?? '', 'utf8')).resolves.toBe('installer');
  });

  it('removes partial downloads after a failed download', async () => {
    const downloadsPath = await tempDir();
    const service = new UpdateService({
      currentVersion: '0.1.0',
      downloadsPath,
      fetchLatestRelease: async () => ({
        status: 200,
        release: release({ tag_name: 'v0.1.1', assets: [asset('Switchify-PC-Setup-0.1.1-x64.exe')] })
      }),
      downloadAsset: async (_asset, destinationPath) => {
        await writeFile(destinationPath, 'partial');
        throw Object.assign(new Error('download failed'), { reason: 'network_error' });
      },
      now: fixedNow
    });
    await service.checkForUpdates();

    const state = await service.downloadUpdate();

    expect(state.download.status).toBe('download_failed');
    await expect(readFile(join(downloadsPath, 'Switchify-PC-Setup-0.1.1-x64.exe.download'))).rejects.toThrow();
  });

  it('uses a numeric suffix when the target filename already exists', async () => {
    const downloadsPath = await tempDir();
    await writeFile(join(downloadsPath, 'Switchify-PC-Setup-0.1.1-x64.exe'), 'existing');
    const service = new UpdateService({
      currentVersion: '0.1.0',
      downloadsPath,
      fetchLatestRelease: async () => ({
        status: 200,
        release: release({ tag_name: 'v0.1.1', assets: [asset('Switchify-PC-Setup-0.1.1-x64.exe')] })
      }),
      downloadAsset: async (_asset, destinationPath, onProgress) => {
        await writeFile(destinationPath, 'new');
        onProgress({ downloadedBytes: 3, totalBytes: 3 });
      },
      now: fixedNow
    });
    await service.checkForUpdates();

    const state = await service.downloadUpdate();

    expect(state.download.filePath).toBe(join(downloadsPath, 'Switchify-PC-Setup-0.1.1-x64 (1).exe'));
  });

  it('returns not_available when there is no available installer to download', async () => {
    const service = new UpdateService({
      currentVersion: '0.1.0',
      downloadsPath: await tempDir(),
      fetchLatestRelease: async () => ({ status: 404, release: null }),
      now: fixedNow
    });
    await service.checkForUpdates();

    const state = await service.downloadUpdate();

    expect(state.download.status).toBe('download_failed');
    expect(state.download.reason).toBe('not_available');
  });
});

describe('selectInstallerAsset', () => {
  it('prefers the exact electron-builder x64 setup artifact', () => {
    expect(
      selectInstallerAsset(
        [
          asset('Other-Setup-0.1.1-x64.exe'),
          asset('Switchify-PC-Setup-0.1.1-x64.exe')
        ],
        '0.1.1'
      )?.name
    ).toBe('Switchify-PC-Setup-0.1.1-x64.exe');
  });

  it('falls back to a setup x64 exe and ignores non-installers', () => {
    expect(
      selectInstallerAsset(
        [
          asset('latest.yml'),
          asset('Switchify-PC-Setup-0.1.1-x64.exe.blockmap'),
          asset('Switchify-PC-Setup-0.1.1-x64.sha256'),
          asset('Switchify Setup x64.exe')
        ],
        '0.1.1'
      )?.name
    ).toBe('Switchify Setup x64.exe');
  });
});

function serviceWithRelease(result: { status: number; release: unknown }): UpdateService {
  return new UpdateService({
    currentVersion: '0.1.0',
    downloadsPath: 'C:\\Downloads',
    fetchLatestRelease: async () => result,
    now: fixedNow
  });
}

function release(overrides: Record<string, unknown> = {}): Record<string, unknown> {
  return {
    tag_name: 'v0.1.0',
    name: 'v0.1.0',
    html_url: 'https://github.com/switchifyapp/switchify-pc/releases/tag/v0.1.0',
    draft: false,
    prerelease: false,
    assets: [],
    ...overrides
  };
}

function asset(name: string): { name: string; browser_download_url: string } {
  return {
    name,
    browser_download_url: `https://github.com/switchifyapp/switchify-pc/releases/download/v0.1.1/${name}`
  };
}

async function tempDir(): Promise<string> {
  const dir = await mkdtemp(join(tmpdir(), 'switchify-update-test-'));
  tempDirs.push(dir);
  return dir;
}

function fixedNow(): Date {
  return new Date('2026-06-12T12:00:00.000Z');
}
