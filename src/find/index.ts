// The settings index: one entry per thing a person might look for, with the
// words they might use for it, where it lives, and what it is set to now.
// Find, Home's common tasks and recently changed, each page's section strip
// and the Also see links all read from this, so there is one map of the app.
import { embed, similarity, type StaticModel } from "./semantic";
import type { AccountView, AppSettings, AppState, BuiltinPanelView, CustomPanel, SwitchProfile } from "../types";
import type { Binding } from "../scanning/useSwitches";
import { actionLabel } from "../scanning/useSwitches";
import { lastModeName, selectOpensHome, type PointScanConfig, type PointScanState } from "../scanning/useScanning";
import { areaOptions, defaultScanPreferences, itemScanning, itemScanningLabels, sharedOptions, type ScanArea } from "../scanning/preferences";
import { speedLevelLabel } from "../settings/pointerSpeedScale";
import { needsInputAccess } from "../inputAccess";

/** The pages of the app. Mouse pointer settings live on Scanning and the account on Settings. */
export type Page = "home" | "switches" | "scanning" | "panels" | "mobile" | "settings" | "support";
export const pageLabels: Record<Page, string> = { home: "Home", switches: "Switches", scanning: "Scanning", panels: "Panels", mobile: "Mobile", settings: "Settings", support: "Help" };

/** Somewhere to go: a page, a section on it, and optionally one setting and the scanning area it is inside. */
export type Target = { page: Page; section: string; setting?: string; area?: ScanArea };

/** Everything the index reads. Null while a part is still loading. */
export type FindContext = {
  state: AppState;
  settings: AppSettings | null;
  config: PointScanConfig | null;
  scan: PointScanState | null;
  bindings: Binding[] | null;
  holdIntervalMs: number | null;
  /** Press timing, in milliseconds. Missing or 0 is off. */
  acceptMs?: number | null;
  ignoreRepeatMs?: number | null;
  panels: CustomPanel[] | null;
  builtins: BuiltinPanelView[];
  profiles: SwitchProfile[] | null;
  account: AccountView | null;
};

export type Entry = {
  id: string;
  label: string;
  /** Other words people use for it, lower case. */
  words: readonly string[];
  page: Page;
  section: string;
  /** A scanning area the setting is inside, opened before jumping. */
  area?: ScanArea;
  /** What it is set to now, in words, or null while unknown. */
  value: (ctx: FindContext) => string | null;
  /** Ids of settings people often want next. */
  related?: readonly string[];
  /** Whether the setting exists on this computer. */
  when?: (ctx: FindContext) => boolean;
  /** Reports state rather than holding a choice, so a change is never the user's. */
  status?: true;
};

export type Section = { id: string; label: string; when?: (ctx: FindContext) => boolean };

const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);
const seconds = (ms: number) => `${ms / 1000} s`;
const auto = (o: { automatic: boolean; intervalMs: number }) => (o.automatic ? `Automatic, every ${seconds(o.intervalMs)}` : "Manual");
const bluetoothWords: Record<AppState["bluetooth"], string> = {
  initializing: "Starting Bluetooth", advertising: "Waiting for a nearby mobile device", connected: "Mobile device connected", poweredOff: "Bluetooth is off",
  unauthorized: "Bluetooth access needed", conflict: "Another Switchify PC is running", unsupported: "Bluetooth unsupported", error: "Bluetooth could not start",
};
const keyboardLayouts: Record<PointScanConfig["keyboardLayout"], string> = { qwerty: "QWERTY", simpleQwerty: "Simple QWERTY", commonLetters: "Common letters first" };

export const sections: Record<Page, Section[]> = {
  home: [],
  switches: [{ id: "switches", label: "Your switches" }, { id: "timing", label: "Press timing" }, { id: "hold", label: "Hold timing" }, { id: "practice", label: "Practice" }],
  scanning: [
    { id: "starting", label: "Starting" }, { id: "areas", label: "Point and Mouse" }, { id: "movement", label: "Movement" }, { id: "after", label: "After a selection" },
    { id: "appearance", label: "Appearance" }, { id: "advanced", label: "Menus and keyboard" }, { id: "pointer", label: "Mouse pointer" },
  ],
  panels: [{ id: "yours", label: "Your panels" }, { id: "builtin", label: "Built-in menus", when: (c) => c.builtins.length > 0 }],
  mobile: [
    { id: "connection", label: "Connection" }, { id: "pairing", label: "Pairing" }, { id: "devices", label: "Paired devices" }, { id: "profiles", label: "Switch Forwarding" },
    { id: "cursor", label: "Cursor overlay", when: (c) => c.state.capabilities.cursorOverlay },
  ],
  settings: [
    { id: "general", label: "General" }, { id: "input", label: "Input" }, { id: "privacy", label: "Privacy" }, { id: "updates", label: "Updates" },
    { id: "licence", label: "Licence" }, { id: "about", label: "About" }, { id: "account", label: "Account", when: (c) => c.account?.available !== false },
  ],
  support: [{ id: "setup", label: "Setup" }, { id: "fix", label: "Fix a problem" }],
};

const catalogue = [
  // Switches
  { id: "switch-list", label: "Your switches", words: ["key", "button", "gamepad", "add switch", "remove", "select", "next", "previous", "assign", "learn", "space", "enter", "remote switch", "usb switch", "jack", "interface", "controller", "name", "edit"], page: "switches", section: "switches",
    value: (c) => c.bindings ? (c.bindings.length ? c.bindings.map((b) => `${b.name || b.key}: ${actionLabel(b.pressAction)}`).join(". ") : "None yet") : null, related: ["hold-interval", "movement", "practice"] },
  { id: "hold-interval", label: "Hold action interval", words: ["hold", "long press", "hold timing", "emergency exit", "escape", "reset"], page: "switches", section: "hold",
    value: (c) => c.holdIntervalMs == null ? null : seconds(c.holdIntervalMs), related: ["switch-list", "scan-rate"] },
  { id: "accept-time", label: "Hold before a press counts", words: ["acceptance time", "tremor", "accidental press", "bump", "too sensitive", "minimum press", "press length", "filter"], page: "switches", section: "timing",
    value: (c) => c.bindings == null ? null : c.acceptMs ? seconds(c.acceptMs) : "Off", related: ["ignore-repeat", "hold-interval"] },
  { id: "ignore-repeat", label: "Ignore presses after a press", words: ["debounce", "double press", "double selection", "bounce", "repeat press", "post acceptance delay", "selects twice"], page: "switches", section: "timing",
    value: (c) => c.bindings == null ? null : c.ignoreRepeatMs ? seconds(c.ignoreRepeatMs) : "Off", related: ["accept-time", "hold-interval"] },
  { id: "practice", label: "Test switches", words: ["practice", "try", "check my switch", "working", "safe"], page: "switches", section: "practice", status: true,
    value: () => "Try your saved switches safely", related: ["switch-list", "not-recognised"] },
  // Scanning
  { id: "select-starts", label: "Select starts", words: ["home menu", "first press", "start", "last mode", "what select opens"], page: "scanning", section: "starting",
    value: (c) => c.config ? (selectOpensHome(c.config) ? "Home menu" : `Last mode used, ${lastModeName(c.config).toLowerCase()}`) : null, related: ["movement", "builtin-menus"] },
  { id: "point-method", label: "Point scanning method", words: ["line", "grid", "line only", "grid then line", "grid size", "line speed", "point"], page: "scanning", section: "areas", area: "point",
    value: (c) => c.config ? `${c.config.mode === "grid" ? `Grid then line, ${c.config.gridSize} by ${c.config.gridSize}` : "Line only"}, speed ${c.config.speed + 1} of 5` : null, related: ["auto-select", "scan-rate"] },
  { id: "auto-select", label: "Auto select", words: ["click options", "auto click", "automatic click", "delay", "dwell"], page: "scanning", section: "areas", area: "point",
    value: (c) => c.config ? (c.config.autoSelectEnabled ? `On, after ${seconds(c.config.autoSelectDelayMs)}` : "Off") : null, related: ["point-method", "dwell"] },
  { id: "radar", label: "Radar speed", words: ["mouse scanning", "radar", "beam", "pointer ring"], page: "scanning", section: "areas", area: "mouse",
    value: (c) => c.config ? (c.config.radarSpeed == null ? "Follows line speed" : `${c.config.radarSpeed + 1} of 5`) : null, related: ["pointer-speed", "repeat"] },
  { id: "movement", label: "Automatic or manual scanning", words: ["auto", "manual", "step", "moves on its own", "next moves"], page: "scanning", section: "movement",
    value: (c) => c.config ? (c.config.automatic ? "Automatic" : "Manual, Next and Previous move the highlight") : null, related: ["scan-rate", "advanced", "switch-list"] },
  { id: "scan-rate", label: "Auto scan rate", words: ["speed", "slower", "faster", "too fast", "too slow", "seconds", "interval", "time on each item", "rate", "step", "tempo", "pace", "dwell time"], page: "scanning", section: "movement",
    value: (c) => c.config ? `Every ${seconds(c.config.blockIntervalMs)}` : null, related: ["movement", "pass-limit", "advanced", "hold-interval"] },
  { id: "first-item", label: "Extra time on the first item", words: ["first item delay", "reaction time", "start delay", "more time", "first row", "first key"], page: "scanning", section: "movement",
    value: (c) => c.config ? (c.config.scanPreferences?.firstItemMs ? seconds(c.config.scanPreferences.firstItemMs) : "Off") : null, related: ["scan-rate", "group-rate"] },
  { id: "group-rate", label: "Inside a row or group", words: ["slower in groups", "item speed", "row speed", "second level", "in group rate", "slower inside"], page: "scanning", section: "movement",
    value: (c) => { if (!c.config) return null; const p = c.config.scanPreferences?.groupRatePercent ?? 100; return p === 100 ? "Same rate" : p === 150 ? "1.5 times as long" : "Twice as long"; }, related: ["scan-rate", "first-item", "item-scanning"] },
  { id: "after-selection", label: "After a selection", words: ["keep scanning", "wait for select", "after a click", "start again from", "where i selected", "beginning"], page: "scanning", section: "after",
    value: (c) => { if (!c.config) return null; const o = sharedOptions(c.config); return `${o.nextScan === "standard" ? "Usual for each scanner" : o.nextScan === "wait" ? "Wait for Select" : "Keep scanning"}. Start from ${o.startFrom === "standard" ? "the usual place" : o.startFrom === "selection" ? "where I selected" : "the beginning"}`; }, related: ["pass-limit", "item-scanning"] },
  { id: "direction", label: "Initial direction", words: ["forward", "reverse", "backwards", "order"], page: "scanning", section: "after",
    value: (c) => c.config ? cap(sharedOptions(c.config).direction) : null, related: ["item-scanning"] },
  { id: "pass-limit", label: "Pass limit", words: ["loops", "how many times", "stop scanning", "passes", "unlimited"], page: "scanning", section: "after",
    value: (c) => { if (!c.config) return null; const n = sharedOptions(c.config).passLimit; return n ? `${n} ${n === 1 ? "pass" : "passes"}` : "Unlimited"; }, related: ["scan-rate"] },
  { id: "item-scanning", label: "Item scanning", words: ["pattern", "groups", "rows", "linear", "one item at a time", "row groups", "group scanning"], page: "scanning", section: "after",
    value: (c) => c.config ? itemScanningLabels[itemScanning(sharedOptions(c.config))] : null, related: ["advanced", "direction"] },
  { id: "colour", label: "Scanner colour", words: ["color", "highlight colour", "red", "green", "blue", "yellow", "outline", "see the highlight", "contrast", "visible"], page: "scanning", section: "appearance",
    value: (c) => c.config ? cap(c.config.scannerColor) : null, related: ["thickness", "overlay-colour", "panel-theme"] },
  { id: "thickness", label: "Highlight thickness", words: ["thin", "thick", "line", "border", "width", "hard to see"], page: "scanning", section: "appearance",
    value: (c) => c.config ? cap(sharedOptions(c.config).thickness) : null, related: ["colour", "panel-size"] },
  { id: "panel-size", label: "Panel size", words: ["bigger", "smaller", "tiles", "large", "small", "text size", "zoom", "keyboard size"], page: "scanning", section: "appearance",
    value: (c) => c.config ? cap(c.config.scanPreferences?.panelSize ?? defaultScanPreferences.panelSize ?? "medium") : null, related: ["panel-theme", "thickness", "your-panels"] },
  { id: "panel-theme", label: "Panel theme", words: ["light", "dark", "dark mode", "light mode", "match system", "contrast", "colours"], page: "scanning", section: "appearance",
    value: (c) => c.config ? ({ system: "Match system", light: "Light", dark: "Dark" })[c.config.scanPreferences?.panelTheme ?? "system"] : null, related: ["panel-size", "colour"] },
  { id: "advanced", label: "Menus and keyboard scanning", words: ["menus", "keyboard speed", "own settings", "override", "different speed", "customise", "advanced"], page: "scanning", section: "advanced",
    value: (c) => c.config ? (["menu", "keyboard"] as ScanArea[]).map((a) => `${a === "menu" ? "Menus" : "Keyboard"}: ${auto(areaOptions(c.config!, a))}`).join(". ") : null, related: ["scan-rate", "item-scanning", "keyboard-layout"] },
  { id: "keyboard-layout", label: "Keyboard layout", words: ["qwerty", "letters", "typing", "simple", "common letters first", "on-screen keyboard", "layout"], page: "scanning", section: "advanced", area: "keyboard",
    value: (c) => c.config ? keyboardLayouts[c.config.keyboardLayout] : null, related: ["word-prediction", "panel-size"] },
  { id: "word-prediction", label: "Word prediction", words: ["suggestions", "predict", "next word", "word completion", "typing", "textslinger"], page: "scanning", section: "advanced", area: "keyboard",
    value: (c) => c.config ? (c.config.wordPrediction ? "On" : "Off") : null, related: ["keyboard-layout"] },
  { id: "panel-position", label: "Move panels away from the pointer", words: ["panel position", "covering", "in the way", "avoid pointer"], page: "scanning", section: "advanced", area: "keyboard",
    value: (c) => c.config ? (c.config.panelAvoidsPointer ? "On" : "Off") : null, related: ["panel-size"] },
  { id: "pointer-speed", label: "Pointer speed", words: ["mouse", "cursor speed", "how far the pointer moves", "slow pointer", "fast pointer", "fine tune"], page: "scanning", section: "pointer",
    value: (c) => c.settings ? `Level ${speedLevelLabel(c.settings.pointerScalePercent)} of 10` : null, related: ["repeat", "radar"] },
  { id: "repeat", label: "Repeat mouse movement and scrolling", words: ["keep moving", "scrolling", "hold to move", "repeat timing", "stop repeating", "release", "press", "acceleration"], page: "scanning", section: "pointer",
    value: (c) => c.settings && c.config ? (c.settings.mouseRepeatEnabled ? `On, every ${seconds(c.settings.moveRepeatIntervalMs)}, stops on switch ${c.config.mouseRepeatStopEdge}` : "Off") : null, related: ["pointer-speed", "hold-interval", "key-repeat"] },
  // Panels
  { id: "your-panels", label: "Your panels", words: ["custom panel", "new panel", "build", "edit panel", "try it", "my panels", "phrase", "tiles"], page: "panels", section: "yours",
    value: (c) => c.panels ? (c.panels.map((p) => p.name).join(", ") || "None yet") : null, related: ["builtin-menus", "panel-size"] },
  { id: "builtin-menus", label: "Built-in menus and panels", words: ["home menu", "editing", "browser", "rearrange", "reset", "order of tiles", "menu tiles", "more actions", "keyboard rows"], page: "panels", section: "builtin", when: (c) => c.builtins.length > 0,
    value: (c) => { const n = c.builtins.filter((b) => b.override).length; return n ? `${n} rearranged` : "None rearranged"; }, related: ["your-panels", "select-starts"] },
  // Mobile
  { id: "connection", label: "Mobile connection", words: ["phone", "tablet", "bluetooth", "android", "remote", "connect", "disconnect", "pc control", "samsung", "pixel"], page: "mobile", section: "connection", status: true,
    value: (c) => c.state.bluetooth === "connected" ? `Connected to ${c.state.connectedDeviceName ?? "a mobile device"}` : bluetoothWords[c.state.bluetooth], related: ["pairing", "devices", "overlay"] },
  { id: "pairing", label: "Connect your mobile", words: ["pair", "code", "approve", "steps", "set up phone", "google play", "install", "qr"], page: "mobile", section: "pairing", status: true,
    value: () => "Three steps, and the app on Google Play", related: ["connection", "devices", "auto-approve"] },
  { id: "devices", label: "Paired devices", words: ["trusted", "forget", "remove phone", "my phone", "last connected"], page: "mobile", section: "devices", status: true,
    value: (c) => `${c.state.pairedDevices.length} paired`, related: ["pairing", "auto-approve"] },
  { id: "profiles", label: "Switch Forwarding profiles", words: ["forwarding", "profile", "remote switches", "map", "phone switches", "forward"], page: "mobile", section: "profiles",
    value: (c) => c.profiles ? `${c.profiles.length} ${c.profiles.length === 1 ? "profile" : "profiles"}` : null, related: ["switch-list", "connection"] },
  { id: "overlay", label: "Cursor overlay", words: ["ring", "pointer ring", "show cursor", "mobile cursor", "crosshairs", "crosshair", "cursor size", "cursor appearance"], page: "mobile", section: "cursor", when: (c) => c.state.capabilities.cursorOverlay,
    value: (c) => c.settings ? (c.settings.cursorOverlayEnabled ? `On, ${c.settings.cursorOverlaySize}${c.settings.cursorCrosshairs ? ", with crosshairs" : ""}` : "Off") : null, related: ["overlay-colour", "connection"] },
  { id: "overlay-colour", label: "Cursor overlay colour", words: ["ring colour", "cursor color", "red", "green", "blue", "yellow", "white"], page: "mobile", section: "cursor", when: (c) => c.state.capabilities.cursorOverlay,
    value: (c) => c.settings ? cap(c.settings.cursorOverlayColor) : null, related: ["overlay", "colour"] },
  // Settings
  { id: "startup", label: "Start with system", words: ["open at login", "launch", "startup", "boot", "sign in", "autostart", "background"], page: "settings", section: "general",
    value: (c) => c.settings ? (c.settings.startWithSystem ? "On" : "Off") : null, related: ["updates"] },
  { id: "dwell", label: "Click when I stop", words: ["dwell", "dwell click", "wait before clicking", "automatic click", "remote pointer"], page: "settings", section: "input",
    value: (c) => c.settings ? (c.settings.dwellClickEnabled ? `On, after ${seconds(c.settings.dwellClickDelayMs)}` : "Off") : null, related: ["auto-select", "repeat"] },
  { id: "key-repeat", label: "Repeat keys", words: ["held keys", "key repeat", "scanned keyboard", "backspace", "arrow keys", "delay before repeating", "key interval"], page: "settings", section: "input",
    value: (c) => c.settings ? `Scanned keyboard ${c.settings.scanKeyRepeatEnabled ? "on" : "off"}, held keys ${c.settings.keyRepeatEnabled ? "on" : "off"}` : null, related: ["repeat"] },
  { id: "privacy", label: "Anonymous diagnostics", words: ["telemetry", "privacy", "share", "data", "analytics", "error reports", "don't share"], page: "settings", section: "privacy",
    value: (c) => ({ undecided: "Not decided yet", enabled: "Sharing", disabled: "Not sharing" })[c.state.telemetry.consent], related: ["diagnostics"] },
  { id: "updates", label: "Updates", words: ["version", "new version", "check for updates", "install", "download", "what version", "update"], page: "settings", section: "updates", status: true,
    value: (c) => `Switchify PC ${c.state.version}, ${({ unconfigured: "updates unavailable in this build", idle: "automatic checks on", checking: "checking", available: "an update is available", downloading: "downloading", readyToInstall: "ready to install", applying: "installing", current: "up to date", failed: "the last check failed", cancelled: "download cancelled" })[c.state.updater.status]}`, related: ["about", "startup"] },
  { id: "licence", label: "Licence", words: ["license", "buy", "trial", "key", "pay", "purchase", "activate", "days left", "expired", "transfer"], page: "settings", section: "licence", status: true,
    value: (c) => c.state.licence?.configured ? c.state.licence.message : "Not needed in this build", related: ["account", "about"] },
  { id: "about", label: "About Switchify PC", words: ["version", "source code", "credits", "open source", "guide", "licence"], page: "settings", section: "about", status: true,
    value: (c) => `Switchify PC ${c.state.version}`, related: ["updates"] },
  { id: "account", label: "Account and settings sync", words: ["sign in", "sign out", "email", "sync", "other computers", "cloud", "log in", "delete account"], page: "settings", section: "account", when: (c) => c.account?.available !== false,
    value: (c) => c.account ? (c.account.signedIn ? `Signed in as ${c.account.email}` : c.account.pendingEmail ? "Waiting for your code" : "Not signed in") : null, related: ["auto-approve", "licence"] },
  { id: "auto-approve", label: "Approve phones on this account automatically", words: ["auto approve", "pairing", "same account", "without codes"], page: "settings", section: "account", when: (c) => c.account?.signedIn === true,
    value: (c) => c.settings ? (c.settings.autoApproveAccountDevices ? "On" : "Off") : null, related: ["account", "pairing"] },
  // Help
  { id: "input-access", label: "Input access", words: ["accessibility", "permission", "allow", "not working", "cannot control", "macos", "system settings"], page: "support", section: "setup", status: true,
    when: (c) => needsInputAccess(c.state),
    value: (c) => ({ granted: "Ready", required: "Permission required", unavailable: "Unavailable on this system" })[c.state.accessibility], related: ["switch-list", "not-recognised"] },
  { id: "not-recognised", label: "Switch not recognized", words: ["not working", "no response", "interface", "keyboard mode", "reserved key", "switch does nothing", "troubleshooting"], page: "support", section: "fix", status: true,
    value: () => "Use a switch interface in keyboard mode", related: ["switch-list", "practice", "input-access"] },
  { id: "diagnostics", label: "Export diagnostics", words: ["logs", "report a problem", "support", "errors", "recent errors", "health", "bluetooth changes"], page: "support", section: "fix", status: true,
    value: (c) => c.state.diagnostics.recentErrors.length ? `${c.state.diagnostics.recentErrors.length} recent errors` : "No recent errors", related: ["privacy", "updates"] },
  { id: "setup-guide", label: "Setup guide", words: ["first run", "walkthrough", "wizard", "getting started", "set up again", "onboarding"], page: "support", section: "setup", status: true,
    value: (c) => (c.state.setup.completed ? "Finished; open it again any time" : "Not finished yet"), related: ["input-access", "switch-list", "startup"] },
  { id: "bluetooth", label: "Bluetooth connection", words: ["disconnect", "bluetooth off", "turn on bluetooth", "another switchify pc", "advertising", "nearby"], page: "support", section: "fix", status: true,
    value: (c) => bluetoothWords[c.state.bluetooth], related: ["connection", "devices"] },
] as const satisfies readonly Entry[];

/** An id the index knows, so a component can only mark itself as a setting that exists. */
export type SettingId = (typeof catalogue)[number]["id"];
export const index: readonly Entry[] = catalogue;

export const entry = (id: string): Entry => index.find((e) => e.id === id)!;
export const targetOf = (e: Entry): Target => ({ page: e.page, section: e.section, setting: e.id, area: e.area });
export const available = (e: Entry, ctx: FindContext) => !e.when || e.when(ctx);
export const sectionLabel = (page: Page, id: string) => sections[page].find((s) => s.id === id)?.label ?? "";

/** Entries matching a query, best first: a label that starts with the word, then one of the words people use for it, then a label containing it, then anything else that mentions it. */
/* --- Matching ------------------------------------------------------------
   Words people use interchangeably. Each group reads as its first word, on
   both the query and the index, so an entry lists one of them and matches
   all. Keep groups to everyday words; the index words carry the rest. */
const synonymGroups: readonly (readonly string[])[] = [
  ["colour", "color"],
  ["phone", "tablet", "mobile", "android", "device", "remote"],
  ["speed", "rate", "interval", "fast", "slow", "faster", "slower", "quick", "quicker", "timing"],
  ["highlight", "scanner", "outline", "marker"],
  ["switch", "button", "key"],
  ["licence", "license", "purchase", "buy", "subscription", "pay"],
  ["start", "startup", "launch", "boot", "autostart", "login"],
  ["size", "bigger", "smaller", "larger", "large", "small", "zoom"],
  ["pair", "pairing", "connect", "connection", "link"],
  ["forget", "remove", "delete", "unpair"],
  ["dark", "light", "theme"],
  ["type", "typing", "keyboard"],
];
const canonical = new Map<string, string>();
for (const group of synonymGroups) for (const word of group) canonical.set(word, group[0]);

/** Lower case, one plural stripped, then its synonym group's first word. */
export function normalise(word: string): string {
  const w = word.normalize("NFD").toLowerCase().replace(/[^a-z0-9]+/g, "");
  if (!w) return "";
  if (canonical.has(w)) return canonical.get(w)!;
  const stem = w.endsWith("ies") && w.length > 4 ? w.slice(0, -3) + "y" : w.endsWith("es") && w.length > 4 && /(ss|x|zz|ch|sh)es$/.test(w) ? w.slice(0, -2) : w.endsWith("s") && !w.endsWith("ss") && w.length > 3 ? w.slice(0, -1) : w;
  return canonical.get(stem) ?? stem;
}
const tokens = (text: string) => text.split(/[^a-zA-Z0-9]+/).map(normalise).filter(Boolean);

/** Within one edit (a typo, a missing or extra letter, two swapped letters) of each other. Short words must match exactly. */
export function close(a: string, b: string): boolean {
  if (a === b) return true;
  if (Math.min(a.length, b.length) < 4 || Math.abs(a.length - b.length) > 1) return false;
  let i = 0;
  while (i < a.length && i < b.length && a[i] === b[i]) i++;
  if (a.length === b.length) {
    // One substitution, or two neighbours swapped.
    return a.slice(i + 1) === b.slice(i + 1) || (a[i] === b[i + 1] && a[i + 1] === b[i] && a.slice(i + 2) === b.slice(i + 2));
  }
  // One letter missing from the shorter word.
  const [long, short] = a.length > b.length ? [a, b] : [b, a];
  return long.slice(i + 1) === short.slice(i);
}

type Indexed = { labelTokens: string[]; wordTokens: string[]; rawTokens: string[]; phrases: string[] };
const indexed = new Map<string, Indexed>();
function indexedOf(e: Entry): Indexed {
  let done = indexed.get(e.id);
  if (!done) {
    const raw = (text: string) => text.toLowerCase().split(/[^a-z0-9]+/).filter((w) => w.length >= 4);
    done = { labelTokens: tokens(e.label), wordTokens: [...new Set(e.words.flatMap(tokens))], rawTokens: [...new Set([e.label, ...e.words].flatMap(raw))], phrases: e.words.map((w) => w.toLowerCase()) };
    indexed.set(e.id, done);
  }
  return done;
}

/**
 * Entries matching a query, best first. Every word of the query must match
 * the entry somewhere: its label, its search words, the page and section it
 * is on, or its current value. Spelling need not be exact, and a plural or a
 * synonym counts, so "colr", "phones" and "quicker" all find their setting.
 */
export function search(query: string, ctx: FindContext): Entry[] {
  const raw = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (!raw.length) return [];
  // A word with no letters or digits (punctuation, an emoji) is not a search
  // term; a query made only of those matches nothing.
  const pairs = raw.map((typed) => [typed, normalise(typed)] as const).filter(([, t]) => t);
  if (!pairs.length) return [];
  const score = (e: Entry) => {
    const { labelTokens, wordTokens, rawTokens, phrases } = indexedOf(e);
    const label = e.label.toLowerCase();
    const haystack = [label, ...phrases, pageLabels[e.page].toLowerCase(), sectionLabel(e.page, e.section).toLowerCase(), (e.value(ctx) ?? "").toLowerCase()];
    const haystackTokens = tokens(haystack.join(" "));
    let total = 0;
    for (const [typed, t] of pairs) {
      if (label.startsWith(typed)) total += 6;
      else if (labelTokens.includes(t) || wordTokens.includes(t)) total += 5;
      else if (label.includes(typed) || phrases.some((p) => p.includes(typed))) total += 4;
      else if (labelTokens.some((w) => w.startsWith(t)) || wordTokens.some((w) => w.startsWith(t))) total += 3;
      else if (labelTokens.some((w) => close(t, w)) || wordTokens.some((w) => close(t, w)) || rawTokens.some((w) => close(typed, w))) total += 2;
      else if (haystackTokens.includes(t) || haystack.some((h) => h.includes(typed))) total += 1;
      else return 0;
      // The word as typed, not a synonym or a stem, breaks ties within a tier.
      if (phrases.includes(typed) || e.label.toLowerCase().split(/[^a-z0-9]+/).includes(typed)) total += 0.25;
    }
    // On a tie, the settings people most often come for rank first.
    return total + (commonTasks.some((task) => task.setting === e.id) ? 0.1 : 0);
  };
  return index.filter((e) => available(e, ctx)).map((e) => ({ e, s: score(e) })).filter((x) => x.s > 0).sort((a, b) => b.s - a.s).map((x) => x.e);
}

/** The tasks people most often come for, each pointing at the exact setting. */
export const commonTasks: { label: string; setting: SettingId }[] = [
  { label: "Make scanning slower or faster", setting: "scan-rate" },
  { label: "Change the highlight colour", setting: "colour" },
  { label: "Add or change a switch", setting: "switch-list" },
  { label: "Make panels bigger", setting: "panel-size" },
  { label: "Connect a phone or tablet", setting: "pairing" },
  { label: "Build a panel for a task", setting: "your-panels" },
];

/** The text that stands for an entry when matching by meaning: what it is called, what people call it, and where it lives. */
export const entryText = (e: Entry) => `${e.label}. ${e.words.join(", ")}. ${pageLabels[e.page]}, ${sectionLabel(e.page, e.section)}.`;

/** Cached, static entry vectors. No current values or user data are embedded. */
export type SemanticIndex = { model: StaticModel; vectors: Map<string, Float32Array | null> };
export const meaningFloor = 0.35;
/** How many meaning-only matches follow the keyword hits: enough to catch the setting, few enough to scan. */
export const meaningLimit = 8;
export function prepareMeaning(model: StaticModel): SemanticIndex {
  return { model, vectors: new Map(index.map((e) => [e.id, embed(model, entryText(e))])) };
}

/** Preserve every keyword hit and its order, then add available close meanings. */
export function searchWithMeaning(query: string, ctx: FindContext, semantic?: SemanticIndex): Entry[] {
  const keyword = search(query, ctx);
  if (!semantic || !/[\p{L}\p{N}]/u.test(query)) return keyword;
  const q = embed(semantic.model, query);
  if (!q) return keyword;
  const seen = new Set(keyword.map((e) => e.id));
  const rest = index.filter((e) => !seen.has(e.id) && available(e, ctx)).map((e) => {
    const v = semantic.vectors.get(e.id);
    return { e, score: v ? similarity(q, v) : -1 };
  }).filter(({ score }) => score >= meaningFloor).sort((a, b) => b.score - a.score).slice(0, meaningLimit);
  return [...keyword, ...rest.map(({ e }) => e)];
}
