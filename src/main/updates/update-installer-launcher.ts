import { existsSync } from 'node:fs';
import { shell } from 'electron';

export type UpdateInstallerLaunchReason = 'installer_unavailable' | 'installer_launch_failed';

export type UpdateInstallerLaunchResult = { ok: true } | { ok: false; reason: UpdateInstallerLaunchReason };

export type OpenPath = (path: string) => Promise<string>;

export async function launchWindowsUpdateInstaller({
  installerPath,
  openPath,
  fileExists = existsSync
}: {
  installerPath: string | null;
  openPath?: OpenPath;
  fileExists?: (path: string) => boolean;
}): Promise<UpdateInstallerLaunchResult> {
  if (!installerPath || !fileExists(installerPath)) {
    return { ok: false, reason: 'installer_unavailable' };
  }

  try {
    const errorMessage = await (openPath ?? shell.openPath)(installerPath);
    if (errorMessage) {
      return { ok: false, reason: 'installer_launch_failed' };
    }
  } catch {
    return { ok: false, reason: 'installer_launch_failed' };
  }

  return { ok: true };
}
