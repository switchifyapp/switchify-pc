import { execFile } from 'node:child_process';

export type StartupApprovedState = 'enabled' | 'disabled' | 'missing' | 'unknown';

export type StartupRegistryEntry = {
  command: string | null;
  startupApproved: StartupApprovedState;
};

export type CommandRunner = (
  file: string,
  args: string[]
) => Promise<{ stdout: string; stderr: string }>;

export type WindowsStartupRegistry = {
  getEntry(valueName: string): Promise<StartupRegistryEntry>;
  setEntry(valueName: string, command: string): Promise<void>;
  deleteEntry(valueName: string): Promise<void>;
};

const RUN_KEY = 'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run';
const STARTUP_APPROVED_RUN_KEY =
  'HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved\\Run';
const STARTUP_APPROVED_ENABLED_HEX = '020000000000000000000000';

export function createWindowsStartupRegistry(commandRunner: CommandRunner = runCommand): WindowsStartupRegistry {
  return {
    async getEntry(valueName) {
      const command = await queryRunCommand(commandRunner, valueName);
      const startupApproved = await queryStartupApproved(commandRunner, valueName);
      return { command, startupApproved };
    },
    async setEntry(valueName, command) {
      await commandRunner('reg.exe', ['add', RUN_KEY, '/v', valueName, '/t', 'REG_SZ', '/d', command, '/f']);
      await commandRunner('reg.exe', [
        'add',
        STARTUP_APPROVED_RUN_KEY,
        '/v',
        valueName,
        '/t',
        'REG_BINARY',
        '/d',
        STARTUP_APPROVED_ENABLED_HEX,
        '/f'
      ]);
    },
    async deleteEntry(valueName) {
      await ignoreMissingValue(commandRunner('reg.exe', ['delete', RUN_KEY, '/v', valueName, '/f']));
      await ignoreMissingValue(commandRunner('reg.exe', ['delete', STARTUP_APPROVED_RUN_KEY, '/v', valueName, '/f']));
    }
  };
}

export function startupCommandFor(executablePath: string, args: string[]): string {
  if (executablePath.includes('"')) {
    throw new Error('Startup executable path cannot contain quotes.');
  }

  for (const arg of args) {
    if (arg.includes('"')) {
      throw new Error('Startup arguments cannot contain quotes.');
    }
  }

  return [`"${executablePath}"`, ...args].join(' ');
}

async function queryRunCommand(commandRunner: CommandRunner, valueName: string): Promise<string | null> {
  try {
    const { stdout } = await commandRunner('reg.exe', ['query', RUN_KEY, '/v', valueName]);
    return parseRegistryValue(stdout, valueName, 'REG_SZ');
  } catch (error) {
    if (isMissingRegistryValueError(error)) return null;
    throw error;
  }
}

async function queryStartupApproved(
  commandRunner: CommandRunner,
  valueName: string
): Promise<StartupApprovedState> {
  try {
    const { stdout } = await commandRunner('reg.exe', ['query', STARTUP_APPROVED_RUN_KEY, '/v', valueName]);
    const value = parseRegistryValue(stdout, valueName, 'REG_BINARY');
    return startupApprovedStateFromHex(value);
  } catch (error) {
    if (isMissingRegistryValueError(error)) return 'missing';
    throw error;
  }
}

function parseRegistryValue(stdout: string, valueName: string, valueType: 'REG_SZ' | 'REG_BINARY'): string | null {
  const escapedName = valueName.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const pattern = new RegExp(`^\\s*${escapedName}\\s+${valueType}\\s+(.+?)\\s*$`, 'im');
  const match = stdout.match(pattern);
  return match?.[1]?.trim() ?? null;
}

function startupApprovedStateFromHex(value: string | null): StartupApprovedState {
  if (!value) return 'unknown';

  const firstByte = value.replace(/\s+/g, '').slice(0, 2).toLowerCase();
  if (firstByte === '02') return 'enabled';
  if (firstByte === '03') return 'disabled';
  return 'unknown';
}

async function ignoreMissingValue(promise: Promise<unknown>): Promise<void> {
  try {
    await promise;
  } catch (error) {
    if (!isMissingRegistryValueError(error)) throw error;
  }
}

function isMissingRegistryValueError(error: unknown): boolean {
  if (!error || typeof error !== 'object') return false;

  const maybeError = error as { code?: unknown; stdout?: unknown; stderr?: unknown; message?: unknown };
  const output = `${String(maybeError.stdout ?? '')}\n${String(maybeError.stderr ?? '')}\n${String(
    maybeError.message ?? ''
  )}`.toLowerCase();

  return maybeError.code === 1 || output.includes('unable to find') || output.includes('cannot find');
}

function runCommand(file: string, args: string[]): Promise<{ stdout: string; stderr: string }> {
  return new Promise((resolve, reject) => {
    execFile(file, args, { windowsHide: true }, (error, stdout, stderr) => {
      if (error) {
        reject(Object.assign(error, { stdout, stderr }));
        return;
      }

      resolve({ stdout, stderr });
    });
  });
}
