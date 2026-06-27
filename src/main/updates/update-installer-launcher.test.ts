import { join } from 'node:path';
import { PassThrough } from 'node:stream';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  launchWindowsUpdateInstaller,
  UPDATE_INSTALLER_ARGS,
  type SpawnInstaller,
  type SpawnProcess
} from './update-installer-launcher';

class FakeSpawnProcess implements SpawnProcess {
  stdout = new PassThrough();
  stderr = new PassThrough();
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

  writeStdout(value: string): void {
    this.stdout.write(value);
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
        fileExists: (path) => path.endsWith('SwitchifyUpdateLauncher.exe')
      })
    ).resolves.toEqual({ ok: false, reason: 'installer_unavailable' });
  });

  it('returns update_launcher_unavailable when the launcher is missing', async () => {
    await expect(
      launch({
        fileExists: (path) => path.endsWith('installer.exe')
      })
    ).resolves.toEqual({ ok: false, reason: 'update_launcher_unavailable' });
  });

  it('spawns the packaged update launcher with installer arguments', async () => {
    const child = new FakeSpawnProcess();
    const spawnInstaller = vi.fn<SpawnInstaller>(() => child);

    const resultPromise = launch({ child, spawnInstaller });
    child.writeStdout('{"ok":true,"status":"installer_started","pid":1234}\n');
    child.emitExit(0);

    await expect(resultPromise).resolves.toEqual({ ok: true, pid: 1234 });
    expect(spawnInstaller).toHaveBeenCalledWith(
      join('resources', 'native', 'SwitchifyUpdateLauncher.exe'),
      ['--installer', join('cache', 'installer.exe'), '--args-json', JSON.stringify(UPDATE_INSTALLER_ARGS)],
      {
        stdio: ['ignore', 'pipe', 'pipe'],
        windowsHide: true
      }
    );
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

  it('returns installer_launch_failed if the child emits error', async () => {
    const child = new FakeSpawnProcess();
    const resultPromise = launch({ child });

    child.emitError();

    await expect(resultPromise).resolves.toEqual({ ok: false, reason: 'installer_launch_failed' });
  });

  it('returns installer_launch_failed if the helper times out', async () => {
    vi.useFakeTimers();
    const child = new FakeSpawnProcess();
    const resultPromise = launch({ child, timeoutMs: 10 });

    await vi.advanceTimersByTimeAsync(10);

    await expect(resultPromise).resolves.toEqual({ ok: false, reason: 'installer_launch_failed' });
  });

  it('maps helper uac_cancelled output', async () => {
    const child = new FakeSpawnProcess();
    const resultPromise = launch({ child });

    child.writeStdout('{"ok":false,"status":"uac_cancelled"}\n');
    child.emitExit(4);

    await expect(resultPromise).resolves.toEqual({ ok: false, reason: 'uac_cancelled' });
  });

  it('maps helper installer_process_unavailable output', async () => {
    const child = new FakeSpawnProcess();
    const resultPromise = launch({ child });

    child.writeStdout('{"ok":false,"status":"installer_process_unavailable"}\n');
    child.emitExit(6);

    await expect(resultPromise).resolves.toEqual({ ok: false, reason: 'installer_process_unavailable' });
  });

  it('returns installer_launch_failed for non-zero exit without usable output', async () => {
    const child = new FakeSpawnProcess();
    const resultPromise = launch({ child });

    child.emitExit(5);

    await expect(resultPromise).resolves.toEqual({ ok: false, reason: 'installer_launch_failed' });
  });

  it('returns update_launcher_invalid_response for invalid success output', async () => {
    const child = new FakeSpawnProcess();
    const resultPromise = launch({ child });

    child.writeStdout('not json\n');
    child.emitExit(0);

    await expect(resultPromise).resolves.toEqual({ ok: false, reason: 'update_launcher_invalid_response' });
  });
});

function launch({
  installerPath = join('cache', 'installer.exe'),
  resourcesPath = 'resources',
  timeoutMs = 1_000,
  child = new FakeSpawnProcess(),
  spawnInstaller = (() => child) as SpawnInstaller,
  fileExists = () => true
}: {
  installerPath?: string | null;
  resourcesPath?: string;
  timeoutMs?: number;
  child?: FakeSpawnProcess;
  spawnInstaller?: SpawnInstaller;
  fileExists?: (path: string) => boolean;
} = {}): Promise<ReturnType<typeof launchWindowsUpdateInstaller> extends Promise<infer Result> ? Result : never> {
  return launchWindowsUpdateInstaller({
    installerPath,
    resourcesPath,
    timeoutMs,
    spawnInstaller,
    fileExists
  });
}
