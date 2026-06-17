import type { SystemStartupSettings } from '../shared/system-startup';

export const START_HIDDEN_ARG = '--start-hidden';

type LoginItemQueryOptions = {
  path: string;
  args: string[];
};

type LoginItemSettingsResult = {
  openAtLogin: boolean;
  executableWillLaunchAtLogin?: boolean;
};

type LoginItemUpdateSettings = {
  openAtLogin: boolean;
  path: string;
  args: string[];
  name: string;
  enabled?: boolean;
};

export type SystemStartupServiceOptions = {
  platform: NodeJS.Platform;
  isPackaged: boolean;
  executablePath: string;
  appUserModelId: string;
  getLoginItemSettings: (options: LoginItemQueryOptions) => LoginItemSettingsResult;
  setLoginItemSettings: (settings: LoginItemUpdateSettings) => void;
};

export function shouldStartHidden(argv: string[], platform: NodeJS.Platform): boolean {
  return platform === 'win32' && argv.includes(START_HIDDEN_ARG);
}

export class SystemStartupService {
  constructor(private readonly options: SystemStartupServiceOptions) {}

  getSettings(): SystemStartupSettings {
    if (!this.isSupported()) {
      return this.unsupportedSettings();
    }

    const loginItemSettings = this.options.getLoginItemSettings(this.loginItemQueryOptions());
    return {
      supported: true,
      startWithSystem: loginItemSettings.openAtLogin && loginItemSettings.executableWillLaunchAtLogin !== false,
      startsHidden: true,
      reason: null
    };
  }

  setStartWithSystem(enabled: boolean): SystemStartupSettings {
    if (!this.isSupported()) {
      return this.unsupportedSettings();
    }

    this.options.setLoginItemSettings({
      openAtLogin: enabled,
      path: this.options.executablePath,
      args: [START_HIDDEN_ARG],
      name: this.options.appUserModelId,
      ...(enabled ? { enabled: true } : {})
    });
    return this.getSettings();
  }

  private isSupported(): boolean {
    return this.options.platform === 'win32' && this.options.isPackaged;
  }

  private loginItemQueryOptions(): LoginItemQueryOptions {
    return {
      path: this.options.executablePath,
      args: [START_HIDDEN_ARG]
    };
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
