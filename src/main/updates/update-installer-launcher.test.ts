import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  launchWindowsUpdateInstaller,
  UPDATE_INSTALLER_ARGS,
  type SpawnInstaller,
  type SpawnProcess
} from './update-installer-launcher';

class FakeSpawnProcess implements SpawnProcess {
  pid = 1234;
  readonly unref = vi.fn();
  private readonly errorListeners: Array<(error: Error) => void> = [];
  private readonly exitListeners: Array<(code: number | null, signal: NodeJS.Signals | null) => void> = [];

  once(event: 'error', listener: (error: Error) => void): void;
  once(event: 'exit', listener: (code: number | null, signal: NodeJS.Signals | null) => void): void;
  once(
    event: 'error' | 'exit',
    listener: ((error: Error) => void) | ((code: number | null, signal: NodeJS.Signals | null) => void)
  ): void {
    if (event === 'error') {
      this.errorListeners.push(listener as (error: Error) => void);
      return;
    }

    this.exitListeners.push(listener as (code: number | null, signal: NodeJS.Signals | null) => void);
  }

  emitError(error = new Error('failed')): void {
    for (const listener of this.errorListeners) {
      listener(error);
    }
  }

  emitExit(code: number | null): void {
    for (const listener of this.exitListeners) {
      listener(code, null);
    }
  }
}

describe('launchWindowsUpdateInstaller', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('returns installer_unavailable when installer path is null', async () => {
    await expect(launch({ installerPath: null })).resolves.toEqual({
      ok: false,
      reason: 'installer_unavailable'
    });
  });

  it('returns installer_unavailable when installer file is missing', async () => {
    await expect(
      launch({
        fileExists: (path) => path.endsWith('elevate.exe')
      })
    ).resolves.toEqual({ ok: false, reason: 'installer_unavailable' });
  });

  it('returns elevation_helper_unavailable when elevate.exe is missing', async () => {
    await expect(
      launch({
        fileExists: (path) => path.endsWith('installer.exe')
      })
    ).resolves.toEqual({ ok: false, reason: 'elevation_helper_unavailable' });
  });

  it('spawns elevate.exe with installer arguments', async () => {
    vi.useFakeTimers();
    const child = new FakeSpawnProcess();
    const spawnInstaller = vi.fn<SpawnInstaller>(() => child);

    const resultPromise = launch({ child, spawnInstaller, settleMs: 10 });
    await vi.advanceTimersByTimeAsync(10);

    await expect(resultPromise).resolves.toEqual({ ok: true, pid: 1234 });
    expect(spawnInstaller).toHaveBeenCalledWith(
      join('resources', 'elevate.exe'),
      [join('cache', 'installer.exe'), ...UPDATE_INSTALLER_ARGS],
      {
        detached: true,
        stdio: 'ignore',
        windowsHide: false
      }
    );
    expect(child.unref).toHaveBeenCalledTimes(1);
  });

  it('returns installer_launch_failed if spawn throws', async () => {
    await expect(
      launch({
        spawnInstaller: () => {
          throw new Error('spawn failed');
        }
      })
    ).resolves.toEqual({ ok: false, reason: 'installer_launch_failed' });
  });

  it('returns installer_launch_failed if the child emits error before settling', async () => {
    vi.useFakeTimers();
    const child = new FakeSpawnProcess();
    const resultPromise = launch({ child, settleMs: 10 });

    child.emitError();

    await expect(resultPromise).resolves.toEqual({ ok: false, reason: 'installer_launch_failed' });
    expect(child.unref).not.toHaveBeenCalled();
  });

  it('returns installer_launch_failed if the child exits non-zero before settling', async () => {
    vi.useFakeTimers();
    const child = new FakeSpawnProcess();
    const resultPromise = launch({ child, settleMs: 10 });

    child.emitExit(1);

    await expect(resultPromise).resolves.toEqual({ ok: false, reason: 'installer_launch_failed' });
    expect(child.unref).not.toHaveBeenCalled();
  });

  it('returns success if the child exits cleanly before settling', async () => {
    vi.useFakeTimers();
    const child = new FakeSpawnProcess();
    const resultPromise = launch({ child, settleMs: 10 });

    child.emitExit(0);

    await expect(resultPromise).resolves.toEqual({ ok: true, pid: 1234 });
    expect(child.unref).toHaveBeenCalledTimes(1);
  });

  it('returns success if the child survives the settle period', async () => {
    vi.useFakeTimers();
    const child = new FakeSpawnProcess();
    const resultPromise = launch({ child, settleMs: 10 });

    await vi.advanceTimersByTimeAsync(10);

    await expect(resultPromise).resolves.toEqual({ ok: true, pid: 1234 });
    expect(child.unref).toHaveBeenCalledTimes(1);
  });
});

function launch({
  installerPath = join('cache', 'installer.exe'),
  resourcesPath = 'resources',
  settleMs = 0,
  child = new FakeSpawnProcess(),
  spawnInstaller = (() => child) as SpawnInstaller,
  fileExists = () => true
}: {
  installerPath?: string | null;
  resourcesPath?: string;
  settleMs?: number;
  child?: FakeSpawnProcess;
  spawnInstaller?: SpawnInstaller;
  fileExists?: (path: string) => boolean;
} = {}): Promise<ReturnType<typeof launchWindowsUpdateInstaller> extends Promise<infer Result> ? Result : never> {
  return launchWindowsUpdateInstaller({
    installerPath,
    resourcesPath,
    settleMs,
    spawnInstaller,
    fileExists
  });
}
