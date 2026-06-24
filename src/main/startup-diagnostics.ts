import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import type { StartupApprovedState } from '../shared/system-startup';

export type StartupDiagnosticsEntry = {
  startedAt: string;
  version: string;
  isPackaged: boolean;
  platform: NodeJS.Platform;
  executablePath: string;
  argv: string[];
  startHidden: boolean;
  startupRegistration?: {
    startWithSystem: boolean;
    registeredCommand: string | null;
    startupApproved: StartupApprovedState;
  };
};

const MAX_STARTUP_DIAGNOSTICS_LINES = 50;

export function appendStartupDiagnostics(filePath: string, entry: StartupDiagnosticsEntry): void {
  try {
    mkdirSync(dirname(filePath), { recursive: true });
    const existingLines = readExistingLines(filePath);
    const nextLines = [...existingLines, JSON.stringify(entry)].slice(-MAX_STARTUP_DIAGNOSTICS_LINES);
    writeFileSync(filePath, `${nextLines.join('\n')}\n`, 'utf8');
  } catch (error) {
    console.warn(error instanceof Error ? error.message : 'Could not write startup diagnostics.');
  }
}

function readExistingLines(filePath: string): string[] {
  try {
    return readFileSync(filePath, 'utf8')
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .filter((line) => {
        try {
          JSON.parse(line);
          return true;
        } catch {
          return false;
        }
      });
  } catch {
    return [];
  }
}
