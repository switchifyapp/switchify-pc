export type SystemStartupUnavailableReason = 'unsupported_platform' | 'unpackaged';

export type SystemStartupSettings = {
  supported: boolean;
  startWithSystem: boolean;
  startsHidden: boolean;
  reason: SystemStartupUnavailableReason | null;
};
