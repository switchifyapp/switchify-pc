import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AccountView, AppSettings, AppState, SwitchProfile, SyncView } from "./types";

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
    keyRepeatEnabled: true, keyRepeatIntervalMs: 250, keyRepeatInitialDelayMs: 500,
    scanKeyRepeatEnabled: false,
    dwellClickEnabled: false, dwellClickDelayMs: 1000,
    cursorOverlayEnabled: true, cursorOverlaySize: "medium", cursorOverlayColor: "red",
    cursorOverlayVisibility: "whileControlling",
    cursorCrosshairs: false, shareDiagnostics: false,
  },
  capabilities: {
    platform: navigator.userAgent.includes("Mac") ? "macos" : "windows",
    grid3: false, uiAccess: false, displayNavigation: false, cursorOverlay: true,
  },
  version: "1.0.0-rc.22",
  diagnostics: { recentBluetooth: [], lastDisconnect: null, recentErrors: [] },
  telemetry: { consent: "undecided", available: true },
  setup: { shown: false, completed: false, autoOpenEligible: true },
  updater: { status: "unconfigured", version: null, downloadedBytes: 0, totalBytes: null, error: null, retryAction: null },
};

const emptyBindings = () => Array.from({ length: 8 }, (_, index) => ({
  switchId: index + 1,
  type: "none" as const,
}));

// Plain browser builds have no backend, so the account is shown as unavailable.
const browserAccount: AccountView = { available: false, signedIn: false, email: null, pendingEmail: null, keychainUnavailable: false };
const accountCall = (command: string, args?: Record<string, unknown>) => "__TAURI_INTERNALS__" in window
  ? invoke<AccountView>(command, args)
  : Promise.resolve(structuredClone(browserAccount));

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
  account: () => accountCall("get_account"),
  /** The account without re-reading the keychain, for background views. */
  accountStatus: () => accountCall("get_account_status"),
  requestSignInCode: (email: string) => accountCall("request_sign_in_code", { email }),
  verifySignInCode: (code: string) => accountCall("verify_sign_in_code", { code }),
  cancelSignIn: () => accountCall("cancel_sign_in"),
  signOut: () => accountCall("sign_out"),
  deleteAccount: () => accountCall("delete_account"),
  settingsSync: () => "__TAURI_INTERNALS__" in window
    ? invoke<SyncView>("get_settings_sync")
    : Promise.resolve<SyncView>({ status: "off", lastSyncedAt: null, message: null }),
  syncSettingsNow: () => invoke<SyncView>("sync_settings_now"),
  resolveSettingsSync: (choice: "local" | "cloud") => invoke<SyncView>("resolve_settings_sync", { choice }),
  onSettingsSync: async (handler: (view: SyncView) => void): Promise<UnlistenFn> => {
    if (!("__TAURI_INTERNALS__" in window)) return () => undefined;
    return listen<SyncView>("settings-sync-changed", (event) => handler(event.payload));
  },
  onProfilesChanged: async (handler: () => void): Promise<UnlistenFn> => {
    if (!("__TAURI_INTERNALS__" in window)) return () => undefined;
    return listen("switch-profiles-changed", () => handler());
  },
  onAccount: async (handler: (account: AccountView) => void): Promise<UnlistenFn> => {
    if (!("__TAURI_INTERNALS__" in window)) return () => undefined;
    return listen<AccountView>("account-changed", (event) => handler(event.payload));
  },
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
