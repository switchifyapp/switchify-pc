export type BluetoothState = "initializing" | "advertising" | "connected" | "poweredOff" | "unauthorized" | "conflict" | "unsupported" | "error";
export type PendingPairing = { requestId: string; deviceId: string; deviceName: string; verificationCode: string; expiresAt: number };
export type PairedDevice = { deviceId: string; deviceName: string; pairedAt: number; lastSeenAt: number | null };
export type Activity = { kind: "info" | "success" | "error"; message: string };
export type DiagnosticEvent = { sequence: number; timestamp: number; category: string; status: string; detail?: string };
export type DiagnosticSummary = {
  recentBluetooth: DiagnosticEvent[];
  lastDisconnect: DiagnosticEvent | null;
  recentErrors: DiagnosticEvent[];
};
export type TelemetryState = { consent: "undecided" | "enabled" | "disabled"; available: boolean };

export type AppSettings = {
  startWithSystem: boolean;
  pointerScalePercent: number;
  mouseRepeatEnabled: boolean;
  moveRepeatIntervalMs: number;
  scrollRepeatIntervalMs: number;
  mouseRepeatAccelerationDurationMs: number;
  cursorOverlayEnabled: boolean;
  cursorOverlaySize: "small" | "medium" | "large";
  cursorOverlayColor: "red" | "green" | "blue" | "yellow" | "white";
  cursorOverlayVisibility: "onInput" | "whileControlling";
  cursorCrosshairs: boolean;
  shareDiagnostics: boolean;
};

export type PlatformCapabilities = {
  platform: "windows" | "macos" | "linux";
  grid3: boolean;
  uiAccess: boolean;
  displayNavigation: boolean;
  cursorOverlay: boolean;
};

export type AppState = {
  bluetooth: BluetoothState;
  accessibility: "granted" | "required" | "unavailable";
  desktopId: string;
  pendingPairings: PendingPairing[];
  pairedDevices: PairedDevice[];
  connectedDeviceName: string | null;
  lastActivity: Activity | null;
  settings: AppSettings;
  capabilities: PlatformCapabilities;
  version: string;
  diagnostics: DiagnosticSummary;
  telemetry: TelemetryState;
  setup: { shown: boolean; completed: boolean };
};

export type SwitchBinding = {
  switchId: number;
  type: "none" | "key" | "mouseButton" | "shortcut" | "mouseClick" | "scroll" | "media";
  value?: string;
  keys?: string[];
  clickCount?: number;
};

export type SwitchProfile = {
  id: string;
  version: number;
  name: string;
  provider: "mapped" | "grid3";
  builtIn: boolean;
  bindings: SwitchBinding[];
};
