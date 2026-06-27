import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';

export type UpdateInstallDiagnosticEvent =
  | 'install_requested'
  | 'confirmation_cancelled'
  | 'installer_missing'
  | 'launcher_missing'
  | 'uac_cancelled'
  | 'installer_launch_failed'
  | 'installer_process_unavailable'
  | 'installer_started'
  | 'cache_cleanup_failed';

export type UpdateInstallDiagnosticEntry = {
  event: UpdateInstallDiagnosticEvent;
  at: string;
  version: string;
  reason?: string;
};

const MAX_DIAGNOSTIC_LINES = 100;

export function appendUpdateInstallDiagnostic(filePath: string, entry: UpdateInstallDiagnosticEntry): void {
  try {
    mkdirSync(dirname(filePath), { recursive: true });
    const existing = readExistingLines(filePath);
    const lines = [...existing, JSON.stringify(entry)].slice(-MAX_DIAGNOSTIC_LINES);
    writeFileSync(filePath, `${lines.join('\n')}\n`, 'utf8');
  } catch (error) {
    console.warn(error instanceof Error ? error.message : 'Switchify update install diagnostics could not be written.');
  }
}

function readExistingLines(filePath: string): string[] {
  try {
    return readFileSync(filePath, 'utf8')
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);
  } catch {
    return [];
  }
}
