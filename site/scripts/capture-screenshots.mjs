// Captures the app screenshots in src/assets/screenshots from the real UI.
// The UI runs in headless Chrome against a fake Tauri bridge with made-up
// data, so nothing here touches the developer's switches, pairing or input.
// See README.md for how to run it.
import puppeteer from "puppeteer-core";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const outDir = fileURLToPath(new URL("../src/assets/screenshots/", import.meta.url));
const url = process.env.APP_URL ?? "http://127.0.0.1:5181/";
const chrome = process.env.CHROME_PATH ?? {
  win32: "C:/Program Files/Google/Chrome/Application/chrome.exe",
  darwin: "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
}[process.platform] ?? "/usr/bin/google-chrome";
// Only the screenshots the docs use are written.
const keep = new Set(["setup-input-access-macos", "setup-add-switch-saved", "setup-start-with-system", "home",
  "add-switch-waiting", "add-switch-learned", "switch-editor", "pairing-request"]);
fs.mkdirSync(outDir, { recursive: true });

const bridge = (platform) => {
  const settings = {
    startWithSystem: false, pointerScalePercent: 100, mouseRepeatEnabled: true,
    moveRepeatIntervalMs: 250, scrollRepeatIntervalMs: 250, mouseRepeatAccelerationDurationMs: 1000,
    keyRepeatEnabled: true, keyRepeatIntervalMs: 250, keyRepeatInitialDelayMs: 500, scanKeyRepeatEnabled: false,
    dwellClickEnabled: false, dwellClickDelayMs: 1000, cursorOverlayEnabled: true, cursorOverlaySize: "medium",
    cursorOverlayColor: "red", cursorOverlayVisibility: "whileControlling", cursorCrosshairs: false,
    shareDiagnostics: false, autoApproveAccountDevices: true, learnWordPrediction: true,
  };
  const app = {
    bluetooth: "advertising", accessibility: platform === "macos" ? "required" : "granted", desktopId: "docs",
    pendingPairings: [], pairedDevices: [], connectedDeviceName: null, lastActivity: null, settings,
    capabilities: { platform, grid3: false, uiAccess: platform === "windows", displayNavigation: true, cursorOverlay: true },
    version: "1.0.0", diagnostics: { recentBluetooth: [], lastDisconnect: null, recentErrors: [] },
    telemetry: { consent: "undecided", available: true },
    setup: { shown: false, completed: false, autoOpenEligible: false },
    updater: { status: "current", version: null, downloadedBytes: 0, totalBytes: null, error: null, retryAction: null },
    switchKeysReleased: false,
  };
  const switches = {
    settings: { schemaVersion: 1, holdIntervalMs: 1000, bindings: [] },
    capture: { active: false, key: null, error: null }, supported: true, error: null,
    escapeHoldMs: 4000, unavailableKeys: [],
  };
  const scanConfig = {
    controlMode: "point", startWith: "home", mouseRepeatStopEdge: "release", wordPrediction: true,
    keyboardLayout: "qwerty", enhancedWordPrediction: false, keyboardWaitAfterTyping: false,
    panelAvoidsPointer: false, scannerColor: "blue", mode: "line", automatic: true, speed: 2, radarSpeed: null,
    gridSize: 4, blockIntervalMs: 1000, autoSelectEnabled: false, autoSelectDelayMs: 1000,
    selectKey: "Space", nextKey: "Enter", backKey: "Backspace", pauseKey: "F8",
  };
  const scan = { config: scanConfig, enabled: true, phase: "idle", paused: false, message: "Ready.", supported: true };
  const remote = { schemaVersion: 1, revision: 1, slots: Array.from({ length: 8 }, () => ({ pressAction: null, holdActions: [] })) };
  const callbacks = new Map();
  const listeners = new Map();
  let nextId = 1;
  const clone = (v) => structuredClone(v);
  const emit = (event, payload) => {
    for (const id of listeners.get(event) ?? []) callbacks.get(id)?.({ event, id, payload });
  };
  const fake = {
    app, switches, scan, remote, emit,
    pushApp() { emit("app-state-changed", clone(app)); },
    pushSwitches() { emit("switches-changed", clone(switches)); },
  };
  window.__fake = fake;
  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => undefined };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main" } },
    transformCallback(cb) { const id = nextId++; callbacks.set(id, cb); return id; },
    unregisterCallback(id) { callbacks.delete(id); },
    convertFileSrc: (p) => p,
    async invoke(cmd, args = {}) {
      switch (cmd) {
        case "plugin:event|listen": {
          const list = listeners.get(args.event) ?? [];
          list.push(args.handler); listeners.set(args.event, list); return args.handler;
        }
        case "plugin:event|unlisten": return null;
        case "get_app_state": case "check_accessibility": case "mark_setup_shown": case "check_for_updates":
          return clone(app);
        case "save_settings": Object.assign(app.settings, args.settings); return clone(app);
        case "complete_setup": app.setup = { shown: true, completed: true, autoOpenEligible: false }; return clone(app);
        case "approve_pairing": case "reject_pairing": return clone(app);
        case "get_switches": return clone(switches);
        case "save_switches": switches.settings = args.settings; switches.capture = { active: false, key: null, error: null }; return clone(switches);
        case "begin_switch_capture": switches.capture = { active: true, key: null, error: null }; return clone(switches);
        case "cancel_switch_capture": switches.capture = { active: false, key: null, error: null }; return clone(switches);
        case "set_switch_keyboard_entry": switches.keyboardEntry = args.active; return clone(switches);
        case "get_point_scan": case "set_keyboard_layout": return clone(scan);
        case "configure_point_scan": Object.assign(scan.config, args.config ?? {}); return clone(scan);
        case "get_remote_switches": case "save_remote_switches": return clone(remote);
        case "list_switch_profiles": return [];
        case "get_account": case "get_account_status":
          return { available: true, signedIn: false, email: null, pendingEmail: null, keychainUnavailable: false };
        case "get_settings_sync": return { status: "off", lastSyncedAt: null, message: null };
        case "take_navigation_request": return null;
        case "get_switch_practice": return null;
        default: return null;
      }
    },
  };
};

const browser = await puppeteer.launch({
  executablePath: chrome,
  headless: true,
  args: ["--hide-scrollbars", "--font-render-hinting=none"],
});

async function open(platform, { width = 1100, height = 760 } = {}) {
  const page = await browser.newPage();
  await page.setViewport({ width, height, deviceScaleFactor: 2 });
  await page.emulateMediaFeatures([
    { name: "prefers-color-scheme", value: "light" },
    { name: "prefers-reduced-motion", value: "reduce" },
  ]);
  await page.evaluateOnNewDocument(bridge, platform);
  await page.goto(url, { waitUntil: "networkidle0" });
  await page.waitForSelector("nav");
  return page;
}

const pause = (ms = 400) => new Promise((r) => setTimeout(r, ms));
async function click(page, text, selector = "button") {
  const handle = await page.waitForFunction((sel, t) =>
    [...document.querySelectorAll(sel)].find((el) => !el.closest("[inert]") && !el.closest("[hidden]") &&
      (el.getAttribute("aria-label") === t || el.textContent.trim() === t)), { timeout: 5000 }, selector, text);
  await handle.asElement().click();
  await pause();
}
async function shot(page, name, selector) {
  if (!keep.has(name)) return;
  await pause(600);
  const file = path.join(outDir, `${name}.png`);
  if (selector) {
    const el = await page.waitForSelector(selector);
    await el.screenshot({ path: file });
  } else {
    await page.screenshot({ path: file });
  }
  console.log("wrote", file);
}
async function fake(page, fn, ...args) { await page.evaluate(fn, ...args); await pause(); }

const headSwitch = { id: "head", name: "Head switch", key: "Space", pressAction: "select", holdActions: [] };

// Setup guide
for (const platform of ["windows", "macos"]) {
  const page = await open(platform, { height: 1000 });
  await click(page, "Help");
  await click(page, "Open setup guide");
  await shot(page, `setup-input-access-${platform}`, ".setup-dialog");
  if (platform === "windows") {
    await click(page, "Next");
    await shot(page, "setup-add-switch-empty", ".setup-dialog");
    await fake(page, (s) => { window.__fake.switches.settings.bindings = [s]; window.__fake.pushSwitches(); }, headSwitch);
    await shot(page, "setup-add-switch-saved", ".setup-dialog");
    await click(page, "Next");
    await click(page, "Next");
    await click(page, "Start with system");
    await shot(page, "setup-start-with-system", ".setup-dialog");
    await click(page, "Next");
    await shot(page, "setup-diagnostics", ".setup-dialog");
  }
  await page.close();
}

// Switches page
{
  const page = await open("windows", { height: 1600 });
  await click(page, "Switches");
  await shot(page, "switches-empty", "main");
  await click(page, "Add switch");
  await page.setViewport({ width: 1100, height: 760, deviceScaleFactor: 2 });
  await shot(page, "add-switch-waiting");
  await page.setViewport({ width: 1100, height: 1600, deviceScaleFactor: 2 });
  await fake(page, () => { window.__fake.switches.capture = { active: false, key: "Space", error: null }; window.__fake.pushSwitches(); });
  await page.type('input[aria-label="New switch name"]', "Head switch");
  await shot(page, "add-switch-learned", ".switch-row[data-open]");
  await page.close();
}
{
  const page = await open("windows", { height: 1600 });
  await fake(page, (s) => {
    window.__fake.switches.settings.bindings = [s, { id: "foot", name: "Foot pedal", key: "F13", pressAction: "next", holdActions: ["back", "stop"] }];
    window.__fake.pushSwitches();
  }, { ...headSwitch, holdActions: ["openKeyboard", "pause"] });
  await click(page, "Switches");
  await shot(page, "switches-saved", "main");
  await click(page, "Edit Head switch");
  await shot(page, "switch-editor", ".switch-row[data-open]");
  await page.close();
}

// Home and pairing
{
  const page = await open("windows");
  await fake(page, (s) => { window.__fake.switches.settings.bindings = [s]; window.__fake.app.setup.completed = true; window.__fake.pushSwitches(); window.__fake.pushApp(); }, headSwitch);
  await shot(page, "home");
  await click(page, "Mobile");
  await shot(page, "mobile");
  await fake(page, () => {
    window.__fake.app.pendingPairings = [{ requestId: "r1", deviceId: "d1", deviceName: "Pixel 8", verificationCode: "482913", expiresAt: Date.now() + 120000 }];
    window.__fake.pushApp();
  });
  await shot(page, "pairing-request");
  await page.close();
}

await browser.close();
