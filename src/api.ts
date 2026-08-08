import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AppSettings, AppState, SwitchProfile } from "./types";

export const browserPreviewState: AppState = {
  bluetooth: "initializing",
  accessibility: "required",
  desktopId: "preview",
  pendingPairing: null,
  pairedDevices: [],
  connectedDeviceName: null,
  lastActivity: null,
  settings: {
    startWithSystem: false, pointerScalePercent: 100, mouseRepeatEnabled: true,
    moveRepeatIntervalMs: 250, scrollRepeatIntervalMs: 250,
    mouseRepeatAccelerationDurationMs: 1000,
    cursorOverlayEnabled: true, cursorOverlaySize: "medium", cursorOverlayColor: "red",
    cursorOverlayVisibility: "whileControlling",
    cursorCrosshairs: false, shareDiagnostics: false,
  },
  capabilities: {
    platform: navigator.userAgent.includes("Mac") ? "macos" : "windows",
    grid3: false, uiAccess: false, displayNavigation: false, cursorOverlay: true,
  },
  version: "0.1.0",
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
  if (!("__TAURI_INTERNALS__" in window)) return structuredClone(browserPreviewState) as T;
  return invoke<T>(command, args);
}

export const api = {
  state: () => call<AppState>("get_app_state"),
  approvePairing: (requestId: string) => call<AppState>("approve_pairing", { requestId }),
  rejectPairing: (requestId: string) => call<AppState>("reject_pairing", { requestId }),
  checkAccessibility: (prompt: boolean) => call<AppState>("check_accessibility", { prompt }),
  disconnectAll: () => call<AppState>("disconnect_all"),
  forgetDevice: (deviceId: string) => call<AppState>("forget_device", { deviceId }),
  saveSettings: (settings: AppSettings) => call<AppState>("save_settings", { settings }),
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
  checkForUpdates: () => call<AppState>("check_for_updates"),
  exportDiagnostics: () => call<AppState>("export_diagnostics"),
  onState: async (handler: (state: AppState) => void): Promise<UnlistenFn> => {
    if (!("__TAURI_INTERNALS__" in window)) return () => undefined;
    return listen<AppState>("preview-state-changed", (event) => handler(event.payload));
  },
};
