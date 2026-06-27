import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { join } from 'node:path';
import type { Readable } from 'node:stream';

export type UpdateInstallerLaunchReason =
  | 'installer_unavailable'
  | 'update_launcher_unavailable'
  | 'uac_cancelled'
  | 'installer_launch_failed'
  | 'installer_process_unavailable'
  | 'update_launcher_invalid_response';

export type UpdateInstallerLaunchResult =
  | { ok: true; pid: number | null }
  | { ok: false; reason: UpdateInstallerLaunchReason };

export type SpawnProcess = {
  stdout: Readable | null;
  stderr: Readable | null;
  once(event: 'error', listener: (error: Error) => void): void;
  once(event: 'exit', listener: (code: number | null, signal: NodeJS.Signals | null) => void): void;
};

export type SpawnInstaller = (
  command: string,
  args: string[],
  options: {
    stdio: ['ignore', 'pipe', 'pipe'];
    windowsHide: true;
  }
) => SpawnProcess;

type HelperResult = {
  ok?: unknown;
  status?: unknown;
  pid?: unknown;
};

export const UPDATE_INSTALLER_ARGS = ['--updated', '--force-run'];
export const UPDATE_LAUNCHER_TIMEOUT_MS = 30_000;
const OUTPUT_LIMIT_BYTES = 8_192;

export async function launchWindowsUpdateInstaller({
  installerPath,
  resourcesPath,
  timeoutMs = UPDATE_LAUNCHER_TIMEOUT_MS,
  spawnInstaller = spawn as SpawnInstaller,
  fileExists = existsSync
}: {
  installerPath: string | null;
  resourcesPath: string;
  timeoutMs?: number;
  spawnInstaller?: SpawnInstaller;
  fileExists?: (path: string) => boolean;
}): Promise<UpdateInstallerLaunchResult> {
  if (!installerPath || !fileExists(installerPath)) {
    return { ok: false, reason: 'installer_unavailable' };
  }

  const launcherPath = join(resourcesPath, 'native', 'SwitchifyUpdateLauncher.exe');
  if (!fileExists(launcherPath)) {
    return { ok: false, reason: 'update_launcher_unavailable' };
  }

  let child: SpawnProcess;
  try {
    child = spawnInstaller(
      launcherPath,
      ['--installer', installerPath, '--args-json', JSON.stringify(UPDATE_INSTALLER_ARGS)],
      {
        stdio: ['ignore', 'pipe', 'pipe'],
        windowsHide: true
      }
    );
  } catch {
    return { ok: false, reason: 'installer_launch_failed' };
  }

  return await waitForLauncherResult(child, timeoutMs);
}

async function waitForLauncherResult(child: SpawnProcess, timeoutMs: number): Promise<UpdateInstallerLaunchResult> {
  let stdout = '';
  let stderr = '';

  child.stdout?.on('data', (chunk: Buffer | string) => {
    stdout = appendLimitedOutput(stdout, chunk);
  });
  child.stderr?.on('data', (chunk: Buffer | string) => {
    stderr = appendLimitedOutput(stderr, chunk);
  });

  return await new Promise((resolve) => {
    let settled = false;
    const timer = setTimeout(() => {
      complete({ ok: false, reason: 'installer_launch_failed' });
    }, timeoutMs);

    const complete = (result: UpdateInstallerLaunchResult): void => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      void stderr;
      resolve(result);
    };

    child.once('error', () => {
      complete({ ok: false, reason: 'installer_launch_failed' });
    });

    child.once('exit', (code) => {
      if (code !== 0) {
        complete({ ok: false, reason: reasonForFailedLauncher(stdout) });
        return;
      }

      complete(parseSuccessfulLauncherOutput(stdout));
    });
  });
}

function parseSuccessfulLauncherOutput(stdout: string): UpdateInstallerLaunchResult {
  const line = stdout
    .split(/\r?\n/)
    .map((value) => value.trim())
    .filter(Boolean)
    .at(-1);
  if (!line) {
    return { ok: false, reason: 'update_launcher_invalid_response' };
  }

  let result: HelperResult;
  try {
    result = JSON.parse(line) as HelperResult;
  } catch {
    return { ok: false, reason: 'update_launcher_invalid_response' };
  }

  if (result.ok === true && result.status === 'installer_started') {
    return {
      ok: true,
      pid: typeof result.pid === 'number' && Number.isFinite(result.pid) ? result.pid : null
    };
  }

  return { ok: false, reason: reasonForStatus(result.status) ?? 'update_launcher_invalid_response' };
}

function reasonForFailedLauncher(stdout: string): UpdateInstallerLaunchReason {
  const line = stdout
    .split(/\r?\n/)
    .map((value) => value.trim())
    .filter(Boolean)
    .at(-1);
  if (!line) {
    return 'installer_launch_failed';
  }

  try {
    const result = JSON.parse(line) as HelperResult;
    return reasonForStatus(result.status) ?? 'installer_launch_failed';
  } catch {
    return 'installer_launch_failed';
  }
}

function reasonForStatus(status: unknown): UpdateInstallerLaunchReason | null {
  switch (status) {
    case 'installer_missing':
      return 'installer_unavailable';
    case 'uac_cancelled':
      return 'uac_cancelled';
    case 'launch_failed':
    case 'unexpected_error':
    case 'invalid_arguments':
      return 'installer_launch_failed';
    case 'installer_process_unavailable':
      return 'installer_process_unavailable';
    default:
      return null;
  }
}

function appendLimitedOutput(current: string, chunk: Buffer | string): string {
  const next = current + chunk.toString();
  if (Buffer.byteLength(next, 'utf8') <= OUTPUT_LIMIT_BYTES) {
    return next;
  }

  return next.slice(-OUTPUT_LIMIT_BYTES);
}
