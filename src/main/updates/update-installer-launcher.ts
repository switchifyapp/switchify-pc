import { existsSync } from 'node:fs';
import { join } from 'node:path';
import { spawn } from 'node:child_process';

export type UpdateInstallerLaunchReason =
  | 'installer_unavailable'
  | 'elevation_helper_unavailable'
  | 'installer_launch_failed';

export type UpdateInstallerLaunchResult =
  | { ok: true; pid: number | null }
  | { ok: false; reason: UpdateInstallerLaunchReason };

export type SpawnProcess = {
  pid?: number;
  unref(): void;
  once(event: 'error', listener: (error: Error) => void): void;
  once(event: 'exit', listener: (code: number | null, signal: NodeJS.Signals | null) => void): void;
};

export type SpawnInstaller = (
  command: string,
  args: string[],
  options: {
    detached: true;
    stdio: 'ignore';
    windowsHide: false;
  }
) => SpawnProcess;

export const UPDATE_INSTALLER_ARGS = ['--updated', '--force-run'];
export const INSTALLER_LAUNCH_SETTLE_MS = 1_500;

export async function launchWindowsUpdateInstaller({
  installerPath,
  resourcesPath,
  settleMs = INSTALLER_LAUNCH_SETTLE_MS,
  spawnInstaller = spawn as SpawnInstaller,
  fileExists = existsSync
}: {
  installerPath: string | null;
  resourcesPath: string;
  settleMs?: number;
  spawnInstaller?: SpawnInstaller;
  fileExists?: (path: string) => boolean;
}): Promise<UpdateInstallerLaunchResult> {
  if (!installerPath || !fileExists(installerPath)) {
    return { ok: false, reason: 'installer_unavailable' };
  }

  const elevationHelperPath = join(resourcesPath, 'elevate.exe');
  if (!fileExists(elevationHelperPath)) {
    return { ok: false, reason: 'elevation_helper_unavailable' };
  }

  let child: SpawnProcess;
  try {
    child = spawnInstaller(elevationHelperPath, [installerPath, ...UPDATE_INSTALLER_ARGS], {
      detached: true,
      stdio: 'ignore',
      windowsHide: false
    });
  } catch {
    return { ok: false, reason: 'installer_launch_failed' };
  }

  return new Promise((resolve) => {
    let settled = false;
    const timer = setTimeout(() => {
      complete({ ok: true, pid: child.pid ?? null });
    }, settleMs);

    const complete = (result: UpdateInstallerLaunchResult): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      if (result.ok) {
        child.unref();
      }
      resolve(result);
    };

    child.once('error', () => {
      complete({ ok: false, reason: 'installer_launch_failed' });
    });
    child.once('exit', (code) => {
      if (code === 0) {
        complete({ ok: true, pid: child.pid ?? null });
        return;
      }

      complete({ ok: false, reason: 'installer_launch_failed' });
    });
  });
}
