import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppSettings, AppState, SwitchProfile } from "./types";

export type ProfileExitAction = "hide" | "quit";
export type NavigationTarget = "home" | "settings" | "profiles";

export const browserState: AppState = {
  bluetooth: "initializing",
  accessibility: "required",
  desktopId: "browser",
  pendingPairings: [],
  pairedDevices: [],
  connectedDeviceName: null,
  lastActivity: null,
  settings: {
    startWithSystem: false, pointerScalePercent: 100, mouseRepeatEnabled: true,
    moveRepeatIntervalMs: 250, scrollRepeatIntervalMs: 250,
    mouseRepeatAccelerationDurationMs: 1000,
    dwellClickEnabled: false, dwellClickDelayMs: 1000,
    cursorOverlayEnabled: true, cursorOverlaySize: "medium", cursorOverlayColor: "red",
    cursorOverlayVisibility: "whileControlling",
    cursorCrosshairs: false, shareDiagnostics: false,
  },
  capabilities: {
    platform: navigator.userAgent.includes("Mac") ? "macos" : "windows",
    grid3: false, uiAccess: false, displayNavigation: false, cursorOverlay: true,
  },
  version: "1.0.0-beta.9",
  diagnostics: { recentBluetooth: [], lastDisconnect: null, recentErrors: [] },
  telemetry: { consent: "undecided", available: true },
  setup: { shown: false, completed: false, autoOpenEligible: true },
  updater: { status: "unconfigured", version: null, downloadedBytes: 0, totalBytes: null, error: null, retryAction: null },
};

const emptyBindings = () => Array.from({ length: 8 }, (_, index) => ({
  switchId: index + 1,
  type: "none" as const,
}));

let browserProfiles: SwitchProfile[] = [{
  id: "builtin.keyboard",
  version: 1,
  name: "Generic keyboard",
  provider: "mapped",
  builtIn: true,
  bindings: emptyBindings(),
}];

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!("__TAURI_INTERNALS__" in window)) return structuredClone(browserState) as T;
  return invoke<T>(command, args);
}

export const api = {
  state: () => call<AppState>("get_app_state"),
  approvePairing: (requestId: string) => call<AppState>("approve_pairing", { requestId }),
  rejectPairing: (requestId: string) => call<AppState>("reject_pairing", { requestId }),
  checkAccessibility: (prompt: boolean) => call<AppState>("check_accessibility", { prompt }),
  disconnectAll: () => call<AppState>("disconnect_all"),
  forgetDevice: (deviceId: string) => call<AppState>("forget_device", { deviceId }),
  saveSettings: (settings: AppSettings) => {
    if ("__TAURI_INTERNALS__" in window) return call<AppState>("save_settings", { settings });
    browserState.settings = structuredClone(settings);
    return Promise.resolve(structuredClone(browserState));
  },
  setTelemetryConsent: (enabled: boolean) => {
    if ("__TAURI_INTERNALS__" in window) return call<AppState>("set_telemetry_consent", { enabled });
    browserState.telemetry = { ...browserState.telemetry, consent: enabled ? "enabled" : "disabled" };
    browserState.settings.shareDiagnostics = enabled;
    return Promise.resolve(structuredClone(browserState));
  },
  markSetupShown: () => {
    if ("__TAURI_INTERNALS__" in window) return call<AppState>("mark_setup_shown");
    browserState.setup.shown = true;
    return Promise.resolve(structuredClone(browserState));
  },
  completeSetup: (startWithSystem: boolean, shareDiagnostics: boolean) => {
    if ("__TAURI_INTERNALS__" in window) return call<AppState>("complete_setup", { startWithSystem, shareDiagnostics });
    if (browserState.pairedDevices.length === 0) return Promise.reject(new Error("Pair an Android device before finishing setup."));
    browserState.settings.startWithSystem = startWithSystem;
    browserState.settings.shareDiagnostics = shareDiagnostics;
    browserState.telemetry.consent = shareDiagnostics ? "enabled" : "disabled";
    browserState.setup = { shown: true, completed: true, autoOpenEligible: false };
    return Promise.resolve(structuredClone(browserState));
  },
  listProfiles: () => "__TAURI_INTERNALS__" in window
    ? call<SwitchProfile[]>("list_switch_profiles")
    : Promise.resolve(structuredClone(browserProfiles)),
  saveProfile: (profile: SwitchProfile) => {
    if ("__TAURI_INTERNALS__" in window) return call<SwitchProfile[]>("save_switch_profile", { profile });
    browserProfiles = [...browserProfiles.filter((item) => item.id !== profile.id), structuredClone(profile)];
    return Promise.resolve(structuredClone(browserProfiles));
  },
  deleteProfile: (profileId: string) => {
    if ("__TAURI_INTERNALS__" in window) return call<SwitchProfile[]>("delete_switch_profile", { profileId });
    browserProfiles = browserProfiles.filter((item) => item.id !== profileId || item.builtIn);
    return Promise.resolve(structuredClone(browserProfiles));
  },
  completeProfileExit: () => "__TAURI_INTERNALS__" in window
    ? invoke<void>("complete_profile_exit")
    : Promise.resolve(),
  cancelProfileExit: () => "__TAURI_INTERNALS__" in window
    ? invoke<void>("cancel_profile_exit")
    : Promise.resolve(),
  checkForUpdates: () => call<AppState>("check_for_updates"),
  downloadUpdate: () => call<AppState>("download_update"),
  cancelUpdateDownload: () => call<AppState>("cancel_update_download"),
  installUpdate: () => call<AppState>("install_update"),
  exportDiagnostics: () => call<AppState>("export_diagnostics"),
  onState: async (handler: (state: AppState) => void): Promise<UnlistenFn> => {
    if (!("__TAURI_INTERNALS__" in window)) return () => undefined;
    return listen<AppState>("app-state-changed", (event) => handler(event.payload));
  },
  onProfileExitRequested: async (handler: (action: ProfileExitAction) => void): Promise<UnlistenFn> => {
    if (!("__TAURI_INTERNALS__" in window)) return () => undefined;
    return listen<ProfileExitAction>("profile-exit-requested", (event) => handler(event.payload));
  },
  onNavigateRequested: async (handler: (target: NavigationTarget) => void): Promise<UnlistenFn> => {
    if (!("__TAURI_INTERNALS__" in window)) return () => undefined;
    const takePending = async () => {
      const target = await invoke<NavigationTarget | null>("take_navigation_request");
      if (target) handler(target);
    };
    const unlisten = await listen<NavigationTarget>("navigate-requested", () => { void takePending(); });
    await takePending();
    return unlisten;
  },
};
