export type SystemStartupUnavailableReason = 'unsupported_platform' | 'unpackaged';
export type StartupApprovedState = 'enabled' | 'disabled' | 'missing' | 'unknown';

export type SystemStartupSettings = {
  supported: boolean;
  startWithSystem: boolean;
  startsHidden: boolean;
  reason: SystemStartupUnavailableReason | null;
  registration?: {
    expectedCommand: string;
    registeredCommand: string | null;
    startupApproved: StartupApprovedState;
  };
};
