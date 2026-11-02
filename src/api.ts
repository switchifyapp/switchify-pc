import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { AccountView, AppSettings, AppState, BuiltinPanelView, CustomPanel, PanelAction, PanelActionEntry, PanelCell, PanelExtrasView, SwitchProfile, SyncView, UpdateProgress } from "./types";

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
    cursorCrosshairs: false, shareDiagnostics: false, autoApproveAccountDevices: true,
    learnWordPrediction: true,
  },
  capabilities: {
    platform: navigator.userAgent.includes("Mac") ? "macos" : "windows",
    grid3: false, uiAccess: false, displayNavigation: false, cursorOverlay: true,
  },
  version: __APP_VERSION__,
  diagnostics: { recentBluetooth: [], lastDisconnect: null, recentErrors: [] },
  telemetry: { consent: "undecided", available: true },
  setup: { shown: false, completed: false, autoOpenEligible: true },
  updater: { status: "unconfigured", version: null, downloadedBytes: 0, totalBytes: null, error: null, retryAction: null },
  switchKeysReleased: false,
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

let browserPanels: CustomPanel[] = [];
// Two of the built-in menus, enough for a plain browser to show the page.
const browserTile = (label: string, action: PanelAction) => ({ label, action, span: 1 });
const browserBuiltinDefaults: BuiltinPanelView[] = [
  { kind: "home", name: "Home", override: null, default: { id: "builtin-home", version: 1, name: "Home", description: "", rows: [
    [browserTile("Point", { type: "openPanel", panel: "point" }), browserTile("Mouse", { type: "openPanel", panel: "mouse" }), browserTile("Keyboard", { type: "openPanel", panel: "keyboard" })],
    [browserTile("Apps and windows", { type: "openPanel", panel: { menu: "windows" } }), browserTile("Editing", { type: "openPanel", panel: { menu: "editing" } }), browserTile("Browser", { type: "openPanel", panel: { menu: "browser" } })],
    [browserTile("Media", { type: "openPanel", panel: { menu: "media" } }), browserTile("Zoom", { type: "openPanel", panel: { menu: "zoom" } })],
    [browserTile("Switchify", { type: "openPanel", panel: "switchify" }), browserTile("My panels", { type: "myPanels" }), browserTile("Close menu", { type: "close" })],
  ] } },
  { kind: "editing", name: "Editing", override: null, default: { id: "builtin-editing", version: 1, name: "Editing", description: "", rows: [
    [browserTile("Copy", { type: "command", command: "copy" }), browserTile("Cut", { type: "command", command: "cut" }), browserTile("Paste", { type: "command", command: "paste" })],
    [browserTile("Select all", { type: "command", command: "selectAll" }), browserTile("Undo", { type: "command", command: "undo" }), browserTile("Redo", { type: "command", command: "redo" })],
    [browserTile("Save", { type: "command", command: "save" }), browserTile("Find", { type: "command", command: "find" })],
    [browserTile("Back", { type: "back" }), browserTile("Close menu", { type: "close" })],
  ] } },
];
let browserBuiltins: BuiltinPanelView[] = structuredClone(browserBuiltinDefaults);
// The mouse panel's and the keyboard's keys, enough for a plain browser to draw them.
const browserKey = (label: string, weight = 1) => ({ label, weight });
const browserKeys = (labels: string, weight = 1) => labels.split("").map((label) => browserKey(label, weight));
const browserExtrasDefaults: PanelExtrasView[] = [
  { target: "mouse", name: "Mouse panel", rows: [], fixed: [
    ["Left click", "Right click", "Double click", "Start drag"].map((label) => browserKey(label)),
    ["↖", "↑", "↗", "Scroll up"].map((label) => browserKey(label)),
    ["←", "Radar", "→"].map((label) => browserKey(label)),
    ["↙", "↓", "↘", "Scroll down"].map((label) => browserKey(label)),
    ["Actions", "More controls", "Keyboard", "Switch to Point"].map((label) => browserKey(label)),
  ] },
  { target: "keyboard", name: "Keyboard", rows: [], fixed: [
    [browserKey("⇥", 1.6), ...browserKeys("qwertyuiop"), browserKey("⌫", 1.9)],
    [browserKey("Caps lock", 1.6), ...browserKeys("asdfghjkl'"), browserKey("↩", 1.9)],
    [browserKey("⬆", 1.6), ...browserKeys("zxcvbnm,./")],
    [browserKey("Ctrl", 1.6), browserKey("Alt", 1.6), browserKey("Space", 4), browserKey("Windows", 1.6)],
    [browserKey("Close keyboard", 1.5), browserKey("Letters •", 1.3), browserKey("Navigation", 1.3), browserKey("Numbers", 1.3), browserKey("Symbols", 1.3), browserKey("Position", 1.5), browserKey("Use simple QWERTY", 1.5)],
  ] },
];
let browserExtras: PanelExtrasView[] = structuredClone(browserExtrasDefaults);
/** Tests share one module; each starts with no panels, the menus as shipped and no extra rows. */
export function resetBrowserPanels() { browserPanels = []; browserBuiltins = structuredClone(browserBuiltinDefaults); browserExtras = structuredClone(browserExtrasDefaults); }

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
  forgetLearnedWords: () => "__TAURI_INTERNALS__" in window ? invoke<boolean>("forget_learned_words") : Promise.resolve(true),
  approvePairing: (requestId: string) => call<AppState>("approve_pairing", { requestId }),
  rejectPairing: (requestId: string) => call<AppState>("reject_pairing", { requestId }),
  checkAccessibility: (prompt: boolean) => call<AppState>("check_accessibility", { prompt }),
  disconnectAll: () => call<AppState>("disconnect_all"),
  setSwitchKeysReleased: (released: boolean) => {
    if ("__TAURI_INTERNALS__" in window) return call<AppState>("set_switch_keys_released", { released });
    browserState.switchKeysReleased = released;
    return Promise.resolve(structuredClone(browserState));
  },
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
  /** The action library for custom panels, filtered by every word of `query` when given. */
  panelActions: (query?: string) => "__TAURI_INTERNALS__" in window
    ? call<PanelActionEntry[]>("get_panel_actions", { query: query ?? null })
    : Promise.resolve([]),
  /** Validates an action built from a template and returns its default tile label. */
  checkPanelAction: (action: PanelAction) => "__TAURI_INTERNALS__" in window
    ? invoke<string>("check_panel_action", { action })
    : Promise.resolve(action.type),
  listPanels: () => "__TAURI_INTERNALS__" in window
    ? call<CustomPanel[]>("list_custom_panels")
    : Promise.resolve(structuredClone(browserPanels)),
  savePanel: (panel: CustomPanel) => {
    if ("__TAURI_INTERNALS__" in window) return call<CustomPanel[]>("save_custom_panel", { panel });
    const existing = browserPanels.find((item) => item.id === panel.id);
    const saved = { ...structuredClone(panel), version: existing ? existing.version + 1 : 1 };
    browserPanels = [...browserPanels.filter((item) => item.id !== panel.id), saved];
    return Promise.resolve(structuredClone(browserPanels));
  },
  deletePanel: (panelId: string) => {
    if ("__TAURI_INTERNALS__" in window) return call<CustomPanel[]>("delete_custom_panel", { panelId });
    browserPanels = browserPanels.filter((item) => item.id !== panelId);
    return Promise.resolve(structuredClone(browserPanels));
  },
  /** The built-in menus, as shipped and as rearranged. */
  listBuiltinPanels: () => "__TAURI_INTERNALS__" in window
    ? call<BuiltinPanelView[]>("list_builtin_panels")
    : Promise.resolve(structuredClone(browserBuiltins)),
  saveBuiltinPanel: (kind: string, panel: CustomPanel) => {
    if ("__TAURI_INTERNALS__" in window) return call<BuiltinPanelView[]>("save_builtin_panel", { kind, panel });
    browserBuiltins = browserBuiltins.map((menu) => menu.kind === kind ? { ...menu, override: { ...structuredClone(panel), version: (menu.override?.version ?? 0) + 1 } } : menu);
    return Promise.resolve(structuredClone(browserBuiltins));
  },
  resetBuiltinPanel: (kind: string) => {
    if ("__TAURI_INTERNALS__" in window) return call<BuiltinPanelView[]>("reset_builtin_panel", { kind });
    browserBuiltins = browserBuiltins.map((menu) => menu.kind === kind ? { ...menu, override: null } : menu);
    return Promise.resolve(structuredClone(browserBuiltins));
  },
  /** The mouse panel and the keyboard with the rows added below their keys. */
  listPanelExtras: () => "__TAURI_INTERNALS__" in window
    ? call<PanelExtrasView[]>("list_panel_extras")
    : Promise.resolve(structuredClone(browserExtras)),
  savePanelExtras: (target: string, rows: PanelCell[][]) => {
    if ("__TAURI_INTERNALS__" in window) return call<PanelExtrasView[]>("save_panel_extras", { target, rows });
    browserExtras = browserExtras.map((view) => view.target === target ? { ...view, rows: structuredClone(rows) } : view);
    return Promise.resolve(structuredClone(browserExtras));
  },
  onPanelsChanged: async (handler: () => void): Promise<UnlistenFn> => {
    if (!("__TAURI_INTERNALS__" in window)) return () => undefined;
    return listen("custom-panels-changed", () => handler());
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
  onUpdateProgress: async (handler: (progress: UpdateProgress) => void): Promise<UnlistenFn> => {
    if (!("__TAURI_INTERNALS__" in window)) return () => undefined;
    return listen<UpdateProgress>("update-progress", (event) => handler(event.payload));
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
