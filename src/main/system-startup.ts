import type { SystemStartupSettings } from '../shared/system-startup';
import { startupCommandFor, type WindowsStartupRegistry } from './windows-startup-registry';

export const START_HIDDEN_ARG = '--start-hidden';
export const STARTUP_VALUE_NAME = 'app.switchify.pc';

export type SystemStartupServiceOptions = {
  platform: NodeJS.Platform;
  isPackaged: boolean;
  executablePath: string;
  startupRegistry: WindowsStartupRegistry;
};

export function shouldStartHidden(argv: string[], platform: NodeJS.Platform): boolean {
  return platform === 'win32' && argv.includes(START_HIDDEN_ARG);
}

export class SystemStartupService {
  constructor(private readonly options: SystemStartupServiceOptions) {}

  async getSettings(): Promise<SystemStartupSettings> {
    if (!this.isSupported()) {
      return this.unsupportedSettings();
    }

    const expectedCommand = this.expectedCommand();
    const entry = await this.getRegistryEntrySafely();

    return {
      supported: true,
      startWithSystem: entry.command === expectedCommand && entry.startupApproved !== 'disabled',
      startsHidden: true,
      reason: null,
      registration: {
        expectedCommand,
        registeredCommand: entry.command,
        startupApproved: entry.startupApproved
      }
    };
  }

  async setStartWithSystem(enabled: boolean): Promise<SystemStartupSettings> {
    if (!this.isSupported()) {
      return this.unsupportedSettings();
    }

    if (enabled) {
      await this.options.startupRegistry.setEntry(STARTUP_VALUE_NAME, this.expectedCommand());
    } else {
      await this.options.startupRegistry.deleteEntry(STARTUP_VALUE_NAME);
    }

    return this.getSettings();
  }

  private isSupported(): boolean {
    return this.options.platform === 'win32' && this.options.isPackaged;
  }

  private expectedCommand(): string {
    return startupCommandFor(this.options.executablePath, [START_HIDDEN_ARG]);
  }

  private async getRegistryEntrySafely(): Promise<{
    command: string | null;
    startupApproved: 'enabled' | 'disabled' | 'missing' | 'unknown';
  }> {
    try {
      return await this.options.startupRegistry.getEntry(STARTUP_VALUE_NAME);
    } catch (error) {
      console.warn(error instanceof Error ? error.message : 'Could not read startup registry settings.');
      return { command: null, startupApproved: 'unknown' };
    }
  }

  private unsupportedSettings(): SystemStartupSettings {
    return {
      supported: false,
      startWithSystem: false,
      startsHidden: true,
      reason: this.options.platform === 'win32' ? 'unpackaged' : 'unsupported_platform'
    };
  }
}
