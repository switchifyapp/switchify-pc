import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { api, browserState, resetBrowserPanels } from "./api";
import type { AccountView, BluetoothState, SwitchProfile } from "./types";

import * as switchHooks from "./scanning/useSwitches";
import * as scanHooks from "./scanning/useScanning";
import { defaultScanPreferences } from "./scanning/preferences";

const defaultBrowserSettings = structuredClone(browserState.settings);

describe("Switchify PC shell", () => {
  beforeEach(() => {
    browserState.accessibility = "required";
    browserState.settings = structuredClone(defaultBrowserSettings);
    browserState.bluetooth = "initializing";
    browserState.pendingPairings = [];
    browserState.pairedDevices = [];
    browserState.connectedDeviceName = null;
    browserState.lastActivity = null;
    browserState.diagnostics = { recentBluetooth: [], lastDisconnect: null, recentErrors: [] };
    browserState.telemetry = { consent: "undecided", available: true };
    browserState.updater = { status: "unconfigured", version: null, downloadedBytes: 0, totalBytes: null, error: null, retryAction: null };
    browserState.setup = { shown: true, completed: false, autoOpenEligible: false };
    browserState.switchKeysReleased = false;
    resetBrowserPanels();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  function localSwitches(): switchHooks.SwitchController {
    const settings: switchHooks.SwitchSettings = { schemaVersion: 1, holdIntervalMs: 1000, bindings: [{ id: "space", name: "Space switch", key: "Space", pressAction: "select", holdActions: [] }] };
    return { settings, state: { settings, capture: { active: false, key: null, error: null }, supported: true, error: null, escapeHoldMs: 4000, unavailableKeys: [] }, pending: 0, error: null, capturing: false, unsaved: false, update: vi.fn(), remove: vi.fn(), capture: vi.fn(), cancelCapture: vi.fn().mockResolvedValue(undefined), setKeyboardEntry: vi.fn().mockResolvedValue(true), retry: vi.fn() };
  }

  it("offers contextual demonstrations without changing app settings", async () => {
    const setSettings = vi.spyOn(api, "saveSettings");
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Mobile" }));
    fireEvent.click(screen.getByRole("button", { name: "Show me how: Pair a mobile device" }));
    expect(screen.getByRole("button", { name: "Pause Pair a mobile device" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Scanning" }));
    expect(screen.queryByRole("button", { name: "Pause Pair a mobile device" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Show me how: Automatic grid then line scanning" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Show me how: Automatic line-only scanning" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Switches" }));
    fireEvent.click(screen.getByRole("button", { name: "More options" }));
    fireEvent.click(screen.getByRole("button", { name: "Switch help" }));
    expect(screen.getByRole("button", { name: "Show me how: Connect a jack switch" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("button", { name: "Show me how: Understand startup behavior" })).toBeInTheDocument();
    expect(setSettings).not.toHaveBeenCalled();
  });

  it("shows local readiness without Bluetooth and explains Mobile suspension", async () => {
    browserState.accessibility = "granted";
    browserState.bluetooth = "unsupported";
    vi.spyOn(switchHooks, "useSwitches").mockReturnValue(localSwitches());
    const scanning: scanHooks.ScanningController = { config: { ...scanHooks.defaultPointScanConfig, scanPreferences: { ...defaultScanPreferences, point: { automatic: false } } }, state: { config: scanHooks.defaultPointScanConfig, supported: true, enabled: true, paused: false, phase: "idle", message: "Ready" }, pending: 0, error: null, unsaved: false, update: vi.fn(), chooseKeyboardLayout: vi.fn(), retry: vi.fn() };
    vi.spyOn(scanHooks, "useScanning").mockReturnValue(scanning);
    let receive: ((state: typeof browserState) => void) | undefined;
    vi.spyOn(api, "onState").mockImplementation(async (handler) => { receive = handler; return () => undefined; });
    render(<App />);
    await screen.findByRole("heading", { name: "Ready" });
    expect(screen.getByText("1 saved. Select assigned")).toBeInTheDocument();
    expect(screen.getByText("Select opens Home. Manual")).toBeInTheDocument();
    scanning.state = { ...scanning.state!, paused: true };
    act(() => receive?.({ ...structuredClone(browserState), bluetooth: "connected" }));
    expect(screen.getByText("Local scanning is paused while a mobile device is connected.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Disconnect" })).toBeInTheDocument();
  });

  it("releases the switch keys from Home and offers to resume them", async () => {
    browserState.accessibility = "granted";
    browserState.bluetooth = "unsupported";
    vi.spyOn(switchHooks, "useSwitches").mockReturnValue(localSwitches());
    const scanning: scanHooks.ScanningController = { config: scanHooks.defaultPointScanConfig, state: { config: scanHooks.defaultPointScanConfig, supported: true, enabled: true, paused: false, phase: "idle", message: "Ready" }, pending: 0, error: null, unsaved: false, update: vi.fn(), chooseKeyboardLayout: vi.fn(), retry: vi.fn() };
    vi.spyOn(scanHooks, "useScanning").mockReturnValue(scanning);
    const release = vi.spyOn(api, "setSwitchKeysReleased");
    render(<App />);
    await screen.findByRole("heading", { name: "Ready" });
    fireEvent.click(screen.getByRole("button", { name: "Pause listening" }));
    expect(release).toHaveBeenCalledWith(true);
    await screen.findByRole("heading", { name: "Listening paused" });
    expect(screen.getByText("1 saved. Listening paused")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Edit switches" })).not.toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Resume listening" })).toHaveLength(2);
    fireEvent.click(screen.getAllByRole("button", { name: "Resume listening" })[0]);
    expect(release).toHaveBeenLastCalledWith(false);
    await screen.findByRole("heading", { name: "Ready" });
    expect(screen.getByRole("button", { name: "Edit switches" })).toBeInTheDocument();
  });

  it("offers switch setup when no Select action is assigned", async () => {
    browserState.accessibility = "granted";
    const switches = localSwitches();
    switches.settings.bindings[0].pressAction = "next";
    vi.spyOn(switchHooks, "useSwitches").mockReturnValue(switches);
    vi.spyOn(scanHooks, "useScanning").mockReturnValue({ config: scanHooks.defaultPointScanConfig, state: { config: scanHooks.defaultPointScanConfig, supported: true, enabled: false, paused: false, phase: "idle", message: "Select needed" }, pending: 0, error: null, unsaved: false, update: vi.fn(), chooseKeyboardLayout: vi.fn(), retry: vi.fn() });
    render(<App />);
    await screen.findByText("Add a switch with the Select action to begin scanning.");
    fireEvent.click(screen.getByRole("button", { name: "Set up switches" }));
    expect(screen.getByRole("heading", { name: "Switches", level: 1 })).toBeInTheDocument();
  });

  it("blocks setup navigation while a switch draft or save needs attention", async () => {
    browserState.setup.autoOpenEligible = true;
    const switches = localSwitches();
    vi.spyOn(switchHooks, "useSwitches").mockReturnValue(switches);
    const { rerender } = render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Next" }));
    fireEvent.click(screen.getByRole("button", { name: "Add switch" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Next" })).toBeDisabled());
    expect(screen.getByRole("button", { name: "Skip switch configuration" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Skip setup for now" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Next" })).toHaveAccessibleDescription("Save or cancel the new switch before continuing.");
    fireEvent.click(screen.getByRole("button", { name: "Cancel new switch" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Next" })).toBeEnabled());
    switches.unsaved = true;
    switches.error = "Save failed";
    rerender(<App />);
    expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Retry save" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Next" })).toHaveAccessibleDescription("Your switch changes have not saved. Retry saving before continuing.");
  });

  it.each([
    ["capture", "Finish capturing a switch or cancel capture before continuing."],
    ["save", "Wait for your switch changes to finish saving before continuing."],
  ])("explains and preserves navigation guards during %s", async (operation, message) => {
    browserState.setup.autoOpenEligible = true;
    const switches = localSwitches();
    vi.spyOn(switchHooks, "useSwitches").mockReturnValue(switches);
    const { rerender } = render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Next" }));
    if (operation === "capture") switches.capturing = true;
    else switches.pending = 1;
    rerender(<App />);
    expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Next" })).toHaveAccessibleDescription(message);
    expect(screen.getByRole("button", { name: "Skip setup for now" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Skip switch configuration" })).toBeDisabled();
    switches.capturing = false;
    switches.pending = 0;
    rerender(<App />);
    expect(screen.getByRole("button", { name: "Next" })).toBeEnabled();
    expect(screen.queryByText(message)).not.toBeInTheDocument();
  });

  it("suspends setup for pairing and returns to the same step", async () => {
    browserState.setup.autoOpenEligible = true;
    const switches = localSwitches();
    vi.spyOn(switchHooks, "useSwitches").mockReturnValue(switches);
    let receive: ((state: typeof browserState) => void) | undefined;
    vi.spyOn(api, "onState").mockImplementation(async (handler) => { receive = handler; return () => undefined; });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Next" }));
    const pending = { ...structuredClone(browserState), pendingPairings: [{ requestId: "one", deviceId: "phone", deviceName: "Phone", verificationCode: "123456", expiresAt: 100 }] };
    switches.state!.capture.active = true;
    act(() => receive?.(pending));
    await screen.findByRole("dialog", { name: "Pairing requests" });
    expect(screen.queryByRole("dialog", { name: "Add your switch" })).not.toBeInTheDocument();
    expect(switches.cancelCapture).toHaveBeenCalled();
    switches.state!.capture.active = false;
    act(() => receive?.({ ...pending, pendingPairings: [] }));
    await waitFor(() => expect(screen.getByRole("dialog", { name: "Add your switch" })).toHaveFocus());
  });

  it("shows pairing failures inside the active pairing dialog", async () => {
    browserState.setup.autoOpenEligible = true;
    browserState.pendingPairings = [{ requestId: "one", deviceId: "phone", deviceName: "Phone", verificationCode: "123456", expiresAt: 100 }];
    vi.spyOn(api, "approvePairing").mockRejectedValue(new Error("Pairing failed. Try again."));
    render(<App />);
    const dialog = await screen.findByRole("dialog", { name: "Pairing requests" });
    fireEvent.click(within(dialog).getByRole("button", { name: /Accept pairing request/ }));
    await waitFor(() => expect(within(dialog).getByRole("alert")).toHaveTextContent("Pairing failed. Try again."));
    expect(within(dialog).getByRole("button", { name: /Accept pairing request/ })).toBeEnabled();
  });

  it("dismisses standalone capture before presenting pairing controls", async () => {
    const switches = localSwitches();
    vi.spyOn(switchHooks, "useSwitches").mockReturnValue(switches);
    let receive: ((state: typeof browserState) => void) | undefined;
    vi.spyOn(api, "onState").mockImplementation(async (handler) => { receive = handler; return () => undefined; });
    switches.capture = vi.fn(async () => { switches.state!.capture.active = true; });
    const { rerender } = render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Switches" }));
    fireEvent.click(screen.getByRole("button", { name: "Add switch" }));
    switches.state!.capture.active = true;
    rerender(<App />);
    await screen.findByRole("dialog", { name: "Press and release your switch" });
    act(() => receive?.({ ...structuredClone(browserState), pendingPairings: [{ requestId: "one", deviceId: "phone", deviceName: "Phone", verificationCode: "123456", expiresAt: 100 }] }));
    const dialog = await screen.findByRole("dialog", { name: "Pairing requests" });
    expect(screen.queryByRole("dialog", { name: "Press and release your switch" })).not.toBeInTheDocument();
    expect(switches.cancelCapture).toHaveBeenCalled();
    const accept = within(dialog).getByRole("button", { name: /Accept pairing request/ });
    accept.focus();
    expect(accept).toHaveFocus();
    fireEvent.keyDown(accept, { key: "Tab" });
    expect(within(dialog).getByRole("button", { name: /Reject pairing request/ })).toHaveFocus();
  });

  it("uses the Switchify application icon in the sidebar", async () => {
    const { container } = render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    const brand = container.querySelector(".brand");
    expect(brand).toHaveTextContent("SwitchifyPC");
    expect(screen.queryByText(new RegExp(["pre", "view"].join(""), "i"))).not.toBeInTheDocument();
    expect(brand?.querySelector("img.brand-mark")).toHaveAttribute("src", expect.stringContaining("icon.png"));
    expect(brand?.querySelector("img.brand-mark")).toHaveAttribute("alt", "");
  });

  it("distinguishes the Mobile install QR from pairing for existing users", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Mobile" }));
    expect(screen.getByRole("heading", { name: "Install Switchify for mobile" })).toBeInTheDocument();
    const qr = screen.getByRole("img", { name: "QR code for Switchify on Google Play" });
    expect(qr.closest("figure")).toHaveTextContent("Get Switchify");
    expect(screen.getByRole("link", { name: "Open Google Play" })).toHaveAttribute("href", expect.stringContaining("play.google.com"));
    const pairing = screen.getByRole("heading", { name: "Connect your mobile" }).closest("section")!;
    expect(within(pairing).getAllByRole("listitem").map((item) => item.textContent)).toEqual([
      "Open Switchify on your mobile device and select this computer.",
      "Compare the pairing codes shown in both apps.",
      "Approve the pairing request on this computer only if the codes match.",
    ]);
  });

  it("shows when paired devices last connected", async () => {
    const connectedAt = Date.UTC(2026, 7, 22, 12, 30);
    browserState.pairedDevices = [
      { deviceId: "phone-1", deviceName: "Pixel", pairedAt: 1, lastSeenAt: connectedAt },
      { deviceId: "phone-2", deviceName: "Tablet", pairedAt: 2, lastSeenAt: null },
    ];

    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Mobile" }));

    expect(screen.getByText(`Last connected ${new Date(connectedAt).toLocaleString()}`)).toBeInTheDocument();
    expect(screen.getByText("Not connected yet")).toBeInTheDocument();
  });

  it("keeps internal activity out of Home and Support while retaining troubleshooting history", async () => {
    browserState.bluetooth = "advertising";
    browserState.lastActivity = { kind: "error", message: "Internal runtime activity" };
    browserState.diagnostics = {
      recentBluetooth: [{ sequence: 1, timestamp: 1, category: "bluetooth", status: "advertising" }],
      lastDisconnect: null,
      recentErrors: [{ sequence: 2, timestamp: 2, category: "runtime", status: "failed", detail: "Bluetooth adapter failed" }],
    };

    render(<App />);

    await screen.findByRole("heading", { name: "Switchify PC" });
    expect(screen.getByText("Control your computer with switches.")).toBeInTheDocument();
    expect(screen.queryByText("Recent activity")).not.toBeInTheDocument();
    expect(screen.queryByText("Internal runtime activity")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Help" }));
    expect(screen.queryByText("Recent activity")).not.toBeInTheDocument();
    expect(screen.queryByText("Internal runtime activity")).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Recent errors" })).toBeInTheDocument();
    expect(screen.getByText("Bluetooth adapter failed")).toBeInTheDocument();
  });

  it.each([
    ["initializing", "Stale tablet", "Preparing this computer for nearby devices."],
    ["advertising", "Stale tablet", "Waiting for a nearby mobile device."],
    ["connected", null, "Mobile device connected."],
    ["connected", "Pixel Tablet", "Pixel Tablet"],
    ["poweredOff", "Stale tablet", "Turn on Bluetooth to connect a mobile device."],
    ["unauthorized", "Stale tablet", "Allow Bluetooth access in System Settings to connect."],
    ["conflict", "Stale tablet", "Quit the other Switchify PC instance, then reopen this app."],
    ["unsupported", "Stale tablet", "This computer does not support the required Bluetooth features."],
    ["error", "Stale tablet", "Bluetooth could not start. Try restarting Switchify PC."],
  ] satisfies Array<[BluetoothState, string | null, string]>)('uses recovery-appropriate Mobile connection copy for Bluetooth state "%s"', async (bluetooth, deviceName, description) => {
    browserState.bluetooth = bluetooth;
    browserState.connectedDeviceName = deviceName;
    browserState.lastActivity = { kind: "error", message: "Internal runtime activity" };

    render(<App />);

    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Mobile" }));
    expect(screen.getByText(description)).toBeInTheDocument();
    if (bluetooth !== "connected") expect(screen.queryByText("Stale tablet")).not.toBeInTheDocument();
    expect(screen.queryByText("Internal runtime activity")).not.toBeInTheDocument();
  });

  it("routes tray navigation without discarding a dirty profile silently", async () => {
    let navigate: ((target: "home" | "settings" | "profiles") => void) | undefined;
    vi.spyOn(api, "onNavigateRequested").mockImplementation(async (handler) => {
      navigate = handler;
      return () => undefined;
    });
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });

    act(() => navigate?.("profiles"));
    fireEvent.click(await screen.findByRole("button", { name: "New profile" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Profile name" }), { target: { value: "Unsaved tray profile" } });
    act(() => navigate?.("settings"));
    expect(confirm).toHaveBeenCalledWith("Discard unsaved profile changes?");
    expect(screen.getByRole("textbox", { name: "Profile name" })).toBeInTheDocument();

    confirm.mockReturnValue(true);
    act(() => navigate?.("settings"));
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
  });

  it("releases tray navigation registered after the app unmounts", async () => {
    let finishRegistration: ((stop: () => void) => void) | undefined;
    let navigate: ((target: "home" | "settings" | "profiles") => void) | undefined;
    vi.spyOn(api, "onNavigateRequested").mockImplementation((handler) => new Promise((resolve) => {
      navigate = handler;
      finishRegistration = resolve;
    }));
    const confirm = vi.spyOn(window, "confirm");
    const stop = vi.fn();
    const rendered = render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });

    act(() => navigate?.("profiles"));
    fireEvent.click(await screen.findByRole("button", { name: "New profile" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Profile name" }), { target: { value: "Pending navigation" } });
    rendered.unmount();

    act(() => navigate?.("settings"));
    expect(confirm).not.toHaveBeenCalled();

    await act(async () => finishRegistration?.(stop));

    expect(stop).toHaveBeenCalledOnce();
  });

  it.each([
    ["available", "Switchify PC 1.0.0-beta.2 is available.", "View update"],
    ["downloading", "Downloading Switchify PC 1.0.0-beta.2 — 25%", "View progress"],
    ["readyToInstall", "Switchify PC 1.0.0-beta.2 is ready to install.", "View update"],
  ] as const)("surfaces the %s updater state and opens the Updates section", async (status, message, action) => {
    browserState.updater = {
      status,
      version: "1.0.0-beta.2",
      downloadedBytes: status === "downloading" ? 50 : 0,
      totalBytes: status === "downloading" ? 200 : null,
      error: null,
      retryAction: null,
    };
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });

    const banner = screen.getByRole("status", { name: "Application update" });
    expect(banner).toHaveTextContent(message);
    fireEvent.click(within(banner).getByRole("button", { name: action }));

    const updates = await screen.findByRole("region", { name: "Updates" });
    await waitFor(() => expect(updates).toHaveFocus());
  });

  it.each(["unconfigured", "idle", "checking", "current", "cancelled", "failed", "applying"] as const)("keeps the %s updater state out of the global banner", async (status) => {
    browserState.updater = {
      status,
      version: status === "unconfigured" || status === "idle" || status === "checking" || status === "current" ? null : "1.0.0-beta.2",
      downloadedBytes: 0,
      totalBytes: null,
      error: status === "failed" ? "Update failed" : null,
      retryAction: status === "failed" ? "check" : null,
    };
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    expect(screen.queryByRole("status", { name: "Application update" })).not.toBeInTheDocument();
  });

  it("shows the licence notice only while the computer is not licensed", async () => {
    const licence = { configured: true, status: "licensed", daysRemaining: 0, licenceId: "id", message: "Licensed for this computer.", storageWarning: false };
    browserState.licence = licence;
    const { unmount } = render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    expect(screen.queryByText(/See Settings, Licence/)).not.toBeInTheDocument();
    unmount();
    browserState.licence = { ...licence, status: "trial", licenceId: null, message: "12 days left in your full trial." };
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    const notice = screen.getByRole("status", { name: "Licence" });
    expect(notice).toHaveTextContent("12 days left in your full trial.");
    expect(notice).toHaveTextContent("See Settings, Licence.");
    delete browserState.licence;
  });

  it("warns a paid user near offline expiry without offering another purchase", async () => {
    browserState.licence = { configured: true, status: "licensed", daysRemaining: 0, licenceId: "id", message: "Connect within 3 days.", storageWarning: false, offlineDaysRemaining: 3 };
    try {
      render(<App />);
      await screen.findByRole("heading", { name: "Switchify PC" });
      expect(screen.getByRole("status", { name: "Licence" })).toHaveTextContent("Connect within 3 days.");
      expect(screen.queryByRole("button", { name: "Buy licence" })).not.toBeInTheDocument();
    } finally { delete browserState.licence; }
  });

  it("renders the connection and permission state", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Switchify PC" })).toBeInTheDocument();
    expect(screen.getByText("Input access")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Accessibility Settings" })).toBeInTheDocument();
  });

  it("opens setup once for a fresh unpaired user and persists dismissal", async () => {
    browserState.setup = { shown: false, completed: false, autoOpenEligible: true };
    const markShown = vi.spyOn(api, "markSetupShown");
    render(<App />);
    const guide = await screen.findByRole("dialog", { name: "Input access" });
    await waitFor(() => expect(guide).toHaveFocus());
    expect(screen.getByLabelText("Step 1 of 5")).toBeInTheDocument();
    await waitFor(() => expect(markShown).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByRole("button", { name: "Skip setup for now" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Input access" })).not.toBeInTheDocument());
    expect(markShown).toHaveBeenCalledTimes(2);
    fireEvent.click(screen.getByRole("button", { name: "Help" }));
    fireEvent.click(screen.getByRole("button", { name: "Open setup guide" }));
    expect(await screen.findByRole("dialog", { name: "Input access" })).toBeInTheDocument();
  });

  it("does not force setup on an existing paired user", async () => {
    browserState.setup = { shown: false, completed: false, autoOpenEligible: false };
    browserState.pairedDevices = [{ deviceId: "phone-1", deviceName: "Pixel", pairedAt: 1, lastSeenAt: null }];
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("does not infer first-run eligibility from a temporarily empty device list", async () => {
    browserState.setup = { shown: false, completed: false, autoOpenEligible: false };
    browserState.pairedDevices = [];
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("completes switch setup without Mobile or Bluetooth and requires explicit choices", async () => {
    browserState.pairedDevices = [];
    browserState.bluetooth = "unsupported";
    const complete = vi.spyOn(api, "completeSetup").mockImplementation(async (startWithSystem, shareDiagnostics) => ({
      ...structuredClone(browserState),
      settings: { ...structuredClone(browserState.settings), startWithSystem, shareDiagnostics },
      telemetry: { ...browserState.telemetry, consent: shareDiagnostics ? "enabled" : "disabled" },
      setup: { shown: true, completed: true, autoOpenEligible: false },
    }));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Help" }));
    fireEvent.click(screen.getByRole("button", { name: "Open setup guide" }));
    fireEvent.click(await screen.findByRole("button", { name: "Next" }));
    expect(screen.getByRole("dialog", { name: "Add your switch" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Next" })).toHaveAccessibleDescription(/If you use only remote switches/);
    fireEvent.click(screen.getByRole("button", { name: "Skip switch configuration" }));
    expect(screen.getByRole("dialog", { name: "Scanning basics" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Next" })).toHaveAccessibleDescription("Choose Start with system or Start manually to enable Next.");
    fireEvent.click(screen.getByRole("button", { name: "Start manually" }));
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByRole("button", { name: "Finish" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Finish" })).toHaveAccessibleDescription("Choose whether to share diagnostics to enable Finish.");
    fireEvent.click(screen.getByRole("button", { name: "Don’t share" }));
    fireEvent.click(screen.getByRole("button", { name: "Finish" }));
    await waitFor(() => expect(complete).toHaveBeenCalledWith(false, false));
  });

  it("shows the compact diagnostic history in troubleshooting", async () => {
    browserState.diagnostics = {
      recentBluetooth: [{ sequence: 1, timestamp: 1, category: "bluetooth", status: "advertising" }],
      lastDisconnect: { sequence: 2, timestamp: 2, category: "disconnect", status: "disconnected", detail: "manual disconnect" },
      recentErrors: [{ sequence: 3, timestamp: 3, category: "runtime", status: "failed", detail: "Bluetooth unavailable" }],
    };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Help" }));
    expect(screen.getByText("Recent Bluetooth changes")).toBeInTheDocument();
    expect(screen.getByText("advertising")).toBeInTheDocument();
    expect(screen.getByText("manual disconnect")).toBeInTheDocument();
    expect(screen.getByText("Bluetooth unavailable")).toBeInTheDocument();
  });

  it("handles simultaneous pairing requests without discarding the queue", async () => {
    browserState.connectedDeviceName = "Connected tablet";
    browserState.pendingPairings = [
      { requestId: "pair-2", deviceId: "android-2", deviceName: "Galaxy", verificationCode: "222222", expiresAt: 2 },
      { requestId: "pair-1", deviceId: "android-1", deviceName: "Pixel", verificationCode: "111111", expiresAt: 1 },
    ];
    const approve = vi.spyOn(api, "approvePairing").mockImplementation(async (requestId) => ({
      ...structuredClone(browserState),
      pendingPairings: browserState.pendingPairings.filter((request) => request.requestId !== requestId),
    }));

    render(<App />);

    const dialog = await screen.findByRole("dialog", { name: "Pairing requests" });
    await waitFor(() => expect(dialog).toHaveFocus());
    expect(screen.getByText("Connected to Connected tablet")).toBeInTheDocument();
    const devices = within(dialog).getAllByRole("heading", { level: 3 });
    expect(devices.map((heading) => heading.textContent)).toEqual(["Galaxy", "Pixel"]);

    fireEvent.click(screen.getByRole("button", { name: "Accept pairing request from Galaxy, code 222222" }));

    await waitFor(() => expect(screen.queryByRole("heading", { name: "Galaxy" })).not.toBeInTheDocument());
    expect(approve).toHaveBeenCalledWith("pair-2");
    expect(screen.getByRole("heading", { name: "Pixel" })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("button", { name: "Reject pairing request from Pixel, code 111111" })).toHaveFocus());
  });

  it("clears a cancelled pairing request from the setup guide runtime event", async () => {
    let stateHandler: ((state: typeof browserState) => void) | undefined;
    browserState.setup = { shown: false, completed: false, autoOpenEligible: true };
    browserState.pendingPairings = [
      { requestId: "pair-cancelled", deviceId: "android-1", deviceName: "Galaxy", verificationCode: "063781", expiresAt: 1 },
    ];
    vi.spyOn(api, "onState").mockImplementation(async (handler) => {
      stateHandler = handler;
      return () => undefined;
    });
    const markShown = vi.spyOn(api, "markSetupShown");

    render(<App />);
    await screen.findByRole("dialog", { name: "Pairing requests" });
    expect(screen.queryByRole("dialog", { name: "Input access" })).not.toBeInTheDocument();
    expect(screen.getByLabelText("Verification code for Galaxy")).toHaveTextContent("063781");
    // Let the auto-open state response settle first, so it cannot land after the cancellation and restore the pairing.
    await waitFor(() => expect(markShown).toHaveBeenCalled());
    await act(() => markShown.mock.results[0].value);

    browserState.pendingPairings = [];
    browserState.lastActivity = { kind: "info", message: "Pairing request cancelled." };
    act(() => stateHandler?.(structuredClone(browserState)));

    expect(await screen.findByRole("dialog", { name: "Input access" })).toBeInTheDocument();
    expect(screen.queryByLabelText("Verification code for Galaxy")).not.toBeInTheDocument();
  });

  it("lists panels with pictures and makes a starter panel", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
    expect(await screen.findByRole("heading", { name: "Panels", level: 1 })).toBeInTheDocument();
    expect(await screen.findByRole("region", { name: "No panels yet" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "New panel" }));
    // The starter panel opens straight away, drawn as the scan will show it.
    expect(await screen.findByRole("heading", { name: "New panel", level: 1 })).toBeInTheDocument();
    const picture = screen.getByRole("group", { name: "New panel as it will scan: Keyboard, Mouse; Back, Close" });
    expect(within(picture).getByRole("button", { name: /^Row 1, tile 1: Keyboard/ })).toBeInTheDocument();
    expect(within(picture).getByRole("button", { name: /^Row 2, tile 2: Close/ })).toHaveAttribute("data-toolbar", "true");

    fireEvent.click(screen.getByRole("button", { name: "All panels" }));
    const card = await screen.findByRole("button", { name: "New panel, 4 tiles" });
    expect(within(card).getByRole("img", { name: "Picture of New panel: Keyboard, Mouse; Back, Close" })).toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "No panels yet" })).not.toBeInTheDocument();
  });

  it("renames, validates and deletes a panel", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
    fireEvent.click(await screen.findByRole("button", { name: "New panel" }));
    await screen.findByRole("heading", { name: "New panel", level: 1 });
    fireEvent.click(screen.getByRole("button", { name: "All panels" }));
    fireEvent.click(await screen.findByRole("button", { name: "New panel" }));
    await screen.findByRole("heading", { name: "New panel 2", level: 1 });

    const name = screen.getByRole("textbox", { name: "Panel name" });
    fireEvent.change(name, { target: { value: " new PANEL " } });
    expect(screen.getByText("Panel names must be unique.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save panel" })).toBeDisabled();
    fireEvent.change(name, { target: { value: "Greetings" } });
    fireEvent.change(screen.getByRole("textbox", { name: "Description" }), { target: { value: "Things I say a lot" } });
    fireEvent.click(screen.getByRole("button", { name: "Save panel" }));
    const card = await screen.findByRole("button", { name: "Greetings, 4 tiles" });
    expect(within(card).getByText("Things I say a lot")).toBeInTheDocument();

    fireEvent.click(card);
    fireEvent.click(await screen.findByRole("button", { name: "Delete panel" }));
    const confirmation = screen.getByRole("alertdialog", { name: "Delete Greetings?" });
    expect(within(confirmation).getByRole("button", { name: "Keep editing" })).toHaveFocus();
    fireEvent.click(within(confirmation).getByRole("button", { name: "Delete panel" }));
    await waitFor(() => expect(screen.queryByRole("button", { name: "Greetings, 4 tiles" })).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: "New panel, 4 tiles" })).toBeInTheDocument();
  });

  it("says so when the built-in menus cannot be listed", async () => {
    vi.spyOn(api, "listBuiltinPanels").mockRejectedValue(new Error("Menu storage unavailable"));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Menu storage unavailable");
  });

  it("rearranges a built-in menu and puts it back", async () => {
    const saveBuiltin = vi.spyOn(api, "saveBuiltinPanel");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
    fireEvent.click(await screen.findByRole("button", { name: "Home, not changed" }));
    expect(await screen.findByRole("heading", { name: "Home", level: 1 })).toBeInTheDocument();
    // The name is fixed, and Reset is off until the menu has been rearranged.
    expect(screen.queryByRole("textbox", { name: "Panel name" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reset to default" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Remove row 1" }));
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    await waitFor(() => expect(saveBuiltin).toHaveBeenCalled());
    expect(saveBuiltin.mock.calls[0][0]).toBe("home");
    expect((saveBuiltin.mock.calls[0][1] as { id: string }).id).toBe("builtin-home");
    expect((saveBuiltin.mock.calls[0][1] as { rows: unknown[] }).rows).toHaveLength(3);
    const rearranged = await screen.findByRole("button", { name: "Home, rearranged" });
    fireEvent.click(rearranged);
    fireEvent.click(await screen.findByRole("button", { name: "Reset to default" }));
    const confirm = screen.getByRole("alertdialog", { name: "Reset Home to default?" });
    fireEvent.click(within(confirm).getByRole("button", { name: "Reset to default" }));
    expect(await screen.findByRole("button", { name: "Home, not changed" })).toBeInTheDocument();
  });

  it("lists the built-in menus and panels under the menu that opens them", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
    // The user's panels come first; the built-ins are one list per opener.
    const builtins = await screen.findByRole("region", { name: "Built-in menus and panels" });
    expect(screen.getByRole("heading", { name: "Panels", level: 1 }).compareDocumentPosition(builtins) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    const home = within(builtins).getByRole("list", { name: "On Home" });
    const names = within(home).getAllByRole("button").map((button) => button.getAttribute("aria-label"));
    expect(names).toEqual(["Home, not changed", "Mouse panel, keys only", "Keyboard, keys only", "Editing, not changed, also on More", "Browser, not changed, also on More", "Tabs, not changed, in Browser"]);
    expect(within(within(home).getByRole("list", { name: "Inside Browser" })).getByRole("button", { name: "Tabs, not changed, in Browser" })).toHaveTextContent("Not changed · In Browser");
    const more = within(builtins).getByRole("list", { name: "On More actions" });
    expect(within(more).getAllByRole("button").map((button) => button.getAttribute("aria-label"))).toEqual(["More actions, not changed", "Displays, not changed"]);
    // Leaving a nested row's editor puts focus back on that row.
    fireEvent.click(within(home).getByRole("button", { name: "Tabs, not changed, in Browser" }));
    await screen.findByRole("heading", { name: "Tabs", level: 1 });
    fireEvent.click(screen.getByRole("button", { name: "All panels" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Tabs, not changed, in Browser" })).toHaveFocus());
  });

  it("keeps a menu it does not know where to place", async () => {
    const stray = { kind: "later", name: "Later", override: null, default: { id: "builtin-later", version: 1, name: "Later", description: "", rows: [[{ label: "Back", action: { type: "back" as const }, span: 1 }]] } };
    vi.spyOn(api, "listBuiltinPanels").mockResolvedValue([stray]);
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
    const other = await screen.findByRole("list", { name: "Other menus" });
    expect(within(other).getByRole("button", { name: "Later, not changed" })).toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "On More actions" })).not.toBeInTheDocument();
  });

  it("keeps a way out of a built-in menu", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
    fireEvent.click(await screen.findByRole("button", { name: "Editing, not changed, also on More" }));
    await screen.findByRole("heading", { name: "Editing", level: 1 });
    fireEvent.click(screen.getByRole("button", { name: "Remove row 4" }));
    expect(screen.getByText("Keep a Back or Close tile so the menu can be left.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save changes" })).toBeDisabled();
  });

  it("adds rows of tiles below the keyboard's keys and removes them again", async () => {
    const saveExtras = vi.spyOn(api, "savePanelExtras");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
    fireEvent.click(await screen.findByRole("button", { name: "Keyboard, keys only" }));
    expect(await screen.findByRole("heading", { name: "Keyboard", level: 1 })).toBeInTheDocument();
    // No name to edit, nothing to remove yet, and no rows is fine as it is.
    expect(screen.queryByRole("textbox", { name: "Panel name" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Remove tiles" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Save changes" })).toBeDisabled();
    expect(screen.getByText("No tiles of your own yet. Add a row below the keys or a column beside them, then give its tiles labels and actions.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add row" }));
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    await waitFor(() => expect(saveExtras).toHaveBeenCalled());
    expect(saveExtras.mock.calls[0][0]).toBe("keyboard");
    expect(saveExtras.mock.calls[0][1]).toHaveLength(1);
    const added = await screen.findByRole("button", { name: "Keyboard, 1 extra row" });
    expect(added).toHaveTextContent("1 extra row");
    fireEvent.click(added);
    fireEvent.click(await screen.findByRole("button", { name: "Remove tiles" }));
    const confirm = screen.getByRole("alertdialog", { name: "Remove the tiles added to Keyboard?" });
    fireEvent.click(within(confirm).getByRole("button", { name: "Remove tiles" }));
    expect(await screen.findByRole("button", { name: "Keyboard, keys only" })).toBeInTheDocument();
    expect(saveExtras.mock.calls[1][1]).toEqual([]);
    expect(saveExtras.mock.calls[1][2]).toEqual([]);
  });

  it("adds a column of tiles beside the mouse panel's keys", async () => {
    const saveExtras = vi.spyOn(api, "savePanelExtras");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
    fireEvent.click(await screen.findByRole("button", { name: "Mouse panel, keys only" }));
    await screen.findByRole("heading", { name: "Mouse panel", level: 1 });
    fireEvent.click(screen.getByRole("button", { name: "Add column" }));
    // The new tile is chosen, in the column, and moves up and down rather than left and right.
    expect(screen.getByRole("heading", { name: "Column 1, tile 1" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add tile to column 1" }));
    expect(screen.getByRole("button", { name: "Move tile up" })).toBeEnabled();
    fireEvent.change(screen.getByRole("textbox", { name: "Label" }), { target: { value: "Second" } });
    expect(screen.getByRole("button", { name: /^Column 1, tile 2: Second/ })).toBeInTheDocument();
    const picture = screen.getByRole("group", { name: "Mouse panel as it will scan: its keys only; columns: Back, Second" });
    expect(within(picture).getByRole("button", { name: /^Column 1, tile 2: Second/ })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Save changes" }));
    await waitFor(() => expect(saveExtras).toHaveBeenCalled());
    expect(saveExtras.mock.calls[0][0]).toBe("mouse");
    expect(saveExtras.mock.calls[0][1]).toEqual([]);
    expect(saveExtras.mock.calls[0][2]).toHaveLength(1);
    expect((saveExtras.mock.calls[0][2] as unknown[][])[0]).toHaveLength(2);
    expect(await screen.findByRole("button", { name: "Mouse panel, 1 column" })).toBeInTheDocument();
    // Two columns at most. Moving a row leaves a chosen column tile where it is,
    // and removing a column puts focus back on Add column.
    fireEvent.click(screen.getByRole("button", { name: "Mouse panel, 1 column" }));
    fireEvent.click(await screen.findByRole("button", { name: "Add column" }));
    expect(screen.getByRole("button", { name: "Add column" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Add row" }));
    fireEvent.click(screen.getByRole("button", { name: "Add row" }));
    fireEvent.click(screen.getByRole("button", { name: /^Column 2, tile 1/ }));
    fireEvent.click(screen.getByRole("button", { name: "Move row 1 down" }));
    expect(screen.getByRole("heading", { name: "Column 2, tile 1" })).toBeInTheDocument();
    // A tile can move to another column, or a new row of its own.
    fireEvent.change(screen.getByRole("combobox", { name: "Move tile to column" }), { target: { value: "0" } });
    expect(screen.getByRole("heading", { name: "Column 1, tile 3" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^Column 2, tile 1/ })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /^Row 1, tile 1/ }));
    fireEvent.change(screen.getByRole("combobox", { name: "Move tile to row" }), { target: { value: "new" } });
    expect(screen.getByRole("heading", { name: "Row 2, tile 1" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Remove column 1" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Add column" })).toHaveFocus());
  });

  it("says what stops a panel saving and goes to the tile", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
    fireEvent.click(await screen.findByRole("button", { name: "New panel" }));
    await screen.findByRole("heading", { name: "New panel", level: 1 });
    expect(screen.getByRole("status")).toBeEmptyDOMElement();
    fireEvent.click(screen.getByRole("button", { name: /^Row 1, tile 2/ }));
    fireEvent.change(screen.getByRole("textbox", { name: "Label" }), { target: { value: "   " } });
    // The note names the tile. Choosing it after moving on selects that tile again.
    const hint = within(screen.getByRole("status")).getByRole("button", { name: "Row 1, tile 2: Give the tile a label." });
    fireEvent.click(screen.getByRole("button", { name: /^Row 2, tile 1/ }));
    expect(screen.getByRole("heading", { name: "Row 2, tile 1" })).toBeInTheDocument();
    fireEvent.click(hint);
    expect(screen.getByRole("heading", { name: "Row 1, tile 2" })).toBeInTheDocument();
    await waitFor(() => expect(screen.getByRole("button", { name: /^Row 1, tile 2: no label/ })).toHaveFocus());
    expect(screen.getByRole("button", { name: "Save panel" })).toBeDisabled();
    // Several problems: a count, going to the first.
    fireEvent.click(screen.getByRole("button", { name: /^Row 2, tile 2/ }));
    fireEvent.change(screen.getByRole("textbox", { name: "Label" }), { target: { value: "   " } });
    expect(within(screen.getByRole("status")).getByRole("button", { name: "2 things need attention. Go to the first" })).toBeInTheDocument();
  });

  it("tries a saved panel in the scanner, but not one with unsaved changes", async () => {
    const tryPanel = vi.spyOn(api, "tryPanel").mockResolvedValue(undefined);
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
    fireEvent.click(await screen.findByRole("button", { name: "New panel" }));
    await screen.findByRole("heading", { name: "New panel", level: 1 });
    fireEvent.click(screen.getByRole("button", { name: "Try it" }));
    await waitFor(() => expect(tryPanel).toHaveBeenCalledTimes(1));
    expect((tryPanel.mock.calls[0][0] as { kind: string }).kind).toBe("panel");
    fireEvent.change(screen.getByRole("textbox", { name: "Panel name" }), { target: { value: "Changed" } });
    expect(screen.getByRole("button", { name: "Try it" })).toBeDisabled();
    // Built-in menus and the native panels try as themselves.
    fireEvent.click(screen.getByRole("button", { name: "All panels" }));
    fireEvent.click(await screen.findByRole("button", { name: "Discard changes" }));
    fireEvent.click(await screen.findByRole("button", { name: "Keyboard, keys only" }));
    fireEvent.click(await screen.findByRole("button", { name: "Try it" }));
    await waitFor(() => expect(tryPanel).toHaveBeenCalledTimes(2));
    expect(tryPanel.mock.calls[1][0]).toEqual({ kind: "native", panel: "keyboard" });
  });

  it("takes only a few rows below the mouse panel's keys", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
    fireEvent.click(await screen.findByRole("button", { name: "Mouse panel, keys only" }));
    await screen.findByRole("heading", { name: "Mouse panel", level: 1 });
    for (let added = 0; added < 4; added++) fireEvent.click(screen.getByRole("button", { name: "Add row" }));
    expect(screen.getByRole("button", { name: "Add row" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Save changes" })).toBeEnabled();
  });

  it("asks before leaving a panel with unsaved changes", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
    fireEvent.click(await screen.findByRole("button", { name: "New panel" }));
    await screen.findByRole("heading", { name: "New panel", level: 1 });
    fireEvent.change(screen.getByRole("textbox", { name: "Panel name" }), { target: { value: "Changed" } });
    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    expect(confirm).toHaveBeenCalledWith("Discard unsaved panel changes?");
    expect(screen.getByRole("heading", { name: "New panel", level: 1 })).toBeInTheDocument();
    confirm.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    expect(await screen.findByRole("heading", { name: "Switchify PC", level: 1 })).toBeInTheDocument();
  });

  it("creates a profile and records a desired key", async () => {
    let saved: SwitchProfile | undefined;
    vi.spyOn(api, "saveProfile").mockImplementation(async (profile) => {
      saved = structuredClone(profile);
      return [profile];
    });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
    const newProfile = await screen.findByRole("button", { name: "New profile" });
    expect(newProfile).toHaveClass("primary");
    fireEvent.click(newProfile);
    fireEvent.change(screen.getByRole("combobox", { name: "Switch 1 action" }), { target: { value: "shortcut" } });
    const recorder = screen.getByRole("textbox", { name: "Switch 1 key" });
    fireEvent.keyDown(recorder, { key: "ArrowUp", ctrlKey: true });
    expect(recorder).toHaveValue("Ctrl + Up Arrow");
    fireEvent.click(screen.getByRole("button", { name: "Save profile" }));
    expect(await screen.findByText("New profile")).toBeInTheDocument();
    expect(saved?.bindings[0].keys).toEqual(["Ctrl", "ArrowUp"]);
  });

  it("duplicates a built-in profile as a uniquely named editable custom profile", async () => {
    const source: SwitchProfile = {
      id: "builtin.keyboard", version: 4, name: "Generic keyboard", provider: "mapped", builtIn: true,
      bindings: Array.from({ length: 8 }, (_, index) => ({ switchId: index + 1, type: index === 0 ? "key" as const : "none" as const, ...(index === 0 ? { value: "Space" } : {}) })),
    };
    vi.spyOn(api, "listProfiles").mockResolvedValue([source, { ...source, id: crypto.randomUUID(), name: "Generic keyboard copy", builtIn: false }]);
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
    fireEvent.click(await screen.findByRole("button", { name: /Generic keyboard.*Built in/ }));
    expect(screen.getByRole("textbox", { name: "Profile name" })).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: "Duplicate" }));

    const name = screen.getByRole("textbox", { name: "Profile name" });
    expect(name).toBeEnabled();
    expect(name).toHaveValue("Generic keyboard copy 2");
    expect(screen.getByRole("textbox", { name: "Switch 1 key" })).toHaveValue("Space");
  });

  it("identifies duplicate names and bindings at their fields", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
    fireEvent.click(await screen.findByRole("button", { name: "New profile" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Profile name" }), { target: { value: "Generic keyboard" } });
    expect(screen.getByText("Profile names must be unique.")).toBeInTheDocument();

    for (const switchId of [1, 2]) {
      fireEvent.change(screen.getByRole("combobox", { name: `Switch ${switchId} action` }), { target: { value: "key" } });
      fireEvent.keyDown(screen.getByRole("textbox", { name: `Switch ${switchId} key` }), { key: " " });
    }

    expect(screen.getByText("This duplicates Switch 2.")).toBeInTheDocument();
    expect(screen.getByText("This duplicates Switch 1.")).toBeInTheDocument();
    expect(screen.getByRole("combobox", { name: "Switch 1 action" })).toHaveAttribute("aria-invalid", "true");
    expect(screen.getByRole("button", { name: "Save profile" })).toBeDisabled();
  });

  it("confirms dirty editor dismissal and restores focus", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
    const opener = await screen.findByRole("button", { name: "New profile" });
    fireEvent.click(opener);
    fireEvent.change(screen.getByRole("textbox", { name: "Profile name" }), { target: { value: "Unsaved controls" } });
    fireEvent.click(screen.getByTitle("Close"));

    const confirmation = screen.getByRole("alertdialog", { name: "Discard unsaved changes?" });
    expect(within(confirmation).getByRole("button", { name: "Keep editing" })).toHaveFocus();
    fireEvent.click(within(confirmation).getByRole("button", { name: "Discard changes" }));
    await waitFor(() => expect(opener).toHaveFocus());
  });

  describe("finding things", () => {
    it("asks before Find discards unsaved panel changes", async () => {
      const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
      render(<App />);
      fireEvent.click(await screen.findByRole("button", { name: "Panels" }));
      fireEvent.click(await screen.findByRole("button", { name: "Home, not changed" }));
      fireEvent.click(await screen.findByRole("button", { name: "Remove row 1" }));
      const find = screen.getByRole("searchbox", { name: "Find a setting" });
      fireEvent.change(find, { target: { value: "s" } });
      expect(confirm).toHaveBeenCalledWith("Discard unsaved panel changes?");
      expect(find).toHaveValue("");
      expect(screen.getByRole("button", { name: "Save changes" })).toBeInTheDocument();
      confirm.mockReturnValue(true);
      fireEvent.change(find, { target: { value: "s" } });
      expect(await screen.findByRole("list", { name: "Matching settings" })).toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
      expect(await screen.findByRole("button", { name: "Home, not changed" })).toBeInTheDocument();
      expect(screen.queryByRole("button", { name: "Save changes" })).not.toBeInTheDocument();
    });

    it("asks before Find discards unsaved profile changes", async () => {
      const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
      render(<App />);
      fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
      fireEvent.click(await screen.findByRole("button", { name: "New profile" }));
      fireEvent.change(screen.getByLabelText("Profile name"), { target: { value: "Break test" } });
      const find = screen.getByRole("searchbox", { name: "Find a setting" });
      fireEvent.change(find, { target: { value: " " } });
      expect(confirm).not.toHaveBeenCalled();
      fireEvent.change(find, { target: { value: "s" } });
      expect(confirm).toHaveBeenCalledWith("Discard unsaved profile changes?");
      expect(screen.getByRole("dialog", { name: "Edit switch profile" })).toBeInTheDocument();
      expect(screen.getByLabelText("Profile name")).toHaveValue("Break test");
    });

    const find = () => screen.getByRole("searchbox", { name: "Find a setting" });

    it("offers every page as a tile in the nav and marks the current one", async () => {
      render(<App />);
      await screen.findByRole("heading", { name: "Switchify PC" });
      const navigation = screen.getByRole("navigation", { name: "Pages" });
      expect(within(navigation).getAllByRole("button").map((button) => button.textContent))
        .toEqual(["Home", "Switches", "Scanning", "Panels", "Mobile", "Settings", "Help"]);
      expect(within(navigation).getByRole("button", { name: "Home" })).toHaveAttribute("aria-current", "page");
      expect(screen.queryByRole("button", { name: "Mouse" })).not.toBeInTheDocument();
      expect(screen.queryByRole("button", { name: /^Menu,/ })).not.toBeInTheDocument();
      expect(document.querySelector("aside")).toBeNull();
      fireEvent.click(within(navigation).getByRole("button", { name: "Switches" }));
      expect(within(navigation).getByRole("button", { name: "Switches" })).toHaveAttribute("aria-current", "page");
      expect(within(navigation).getByRole("button", { name: "Home" })).not.toHaveAttribute("aria-current");
    });

    it("finds settings by the words people use and takes them straight there", async () => {
      render(<App />);
      await screen.findByRole("heading", { name: "Switchify PC" });
      fireEvent.change(find(), { target: { value: "speed" } });

      expect(screen.getByRole("heading", { name: /settings match “speed”/, level: 1 })).toBeInTheDocument();
      const results = within(screen.getByRole("list", { name: "Matching settings" })).getAllByRole("button");
      expect(results[0]).toHaveAccessibleName(/^Auto scan rate, Scanning, Movement, Every/);
      expect(results.map((button) => button.getAttribute("aria-label")!.split(",")[0])).toContain("Pointer speed");
      expect(screen.queryByRole("heading", { name: "Switchify PC", level: 1 })).not.toBeInTheDocument();
      expect(screen.getByRole("navigation", { name: "Pages" })).toBeInTheDocument();

      fireEvent.click(results[0]);
      expect(find()).toHaveValue("");
      expect(await screen.findByRole("heading", { name: "Scanning", level: 1 })).toBeInTheDocument();
      const rate = await waitFor(() => { const el = document.querySelector('[data-setting="scan-rate"]'); expect(el).toHaveAttribute("data-found", "true"); return el!; }, { timeout: 3000 });
      expect(rate).toContainElement(document.activeElement as HTMLElement);
      expect(screen.getByRole("button", { name: "Scanning" })).toHaveAttribute("aria-current", "page");
    });

    it("clears the search, and falls back to common tasks when nothing matches", async () => {
      render(<App />);
      await screen.findByRole("heading", { name: "Switchify PC" });
      fireEvent.change(find(), { target: { value: "zzzq" } });
      expect(await screen.findByRole("heading", { name: "Nothing matches “zzzq”", level: 1 })).toBeInTheDocument();
      const list = screen.getByRole("list", { name: "Matching settings" });
      expect(within(list).getAllByRole("button")[0]).toHaveAccessibleName(/^Make scanning slower or faster, Scanning, Movement/);
      fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
      expect(find()).toHaveValue("");
      expect(find()).toHaveFocus();
      expect(screen.getByRole("heading", { name: "Switchify PC", level: 1 })).toBeInTheDocument();
    });

    it("focuses Find on a slash unless a field has focus", async () => {
      render(<App />);
      await screen.findByRole("heading", { name: "Switchify PC" });
      fireEvent.keyDown(document.body, { key: "/" });
      expect(find()).toHaveFocus();
      fireEvent.click(screen.getByRole("button", { name: "Settings" }));
      const startup = screen.getByRole("checkbox", { name: "Start with system" });
      startup.focus();
      fireEvent.keyDown(startup, { key: "/" });
      expect(startup).toHaveFocus();
    });

    it("opens the exact setting from a common task on Home", async () => {
      render(<App />);
      await screen.findByRole("heading", { name: "Switchify PC" });
      const tasks = screen.getByRole("heading", { name: "Common tasks", level: 2 }).closest("section")!;
      fireEvent.click(within(tasks).getByRole("button", { name: /^Change the highlight colour, Scanning, Appearance/ }));
      expect(await screen.findByRole("heading", { name: "Scanning", level: 1 })).toBeInTheDocument();
      // Appearance sits behind More options, which the jump opens.
      await waitFor(() => expect(document.querySelector('[data-setting="colour"]')).toHaveAttribute("data-found", "true"), { timeout: 3000 });
      expect(screen.getByRole("radio", { name: "Red" })).toBeVisible();
    });

    it("jumps to a section from the page strip, opening More options when it hides the section", async () => {
      render(<App />);
      fireEvent.click(await screen.findByRole("button", { name: "Switches" }));
      const strip = screen.getByRole("navigation", { name: "On this page" });
      expect(within(strip).getAllByRole("button").map((button) => button.getAttribute("aria-label")!.split(",")[0])).toEqual(["Your switches", "Hold timing", "Practice"]);
      expect(within(strip).getByRole("button", { name: /^Hold timing/ })).toHaveAccessibleName("Hold timing, 1 s");
      expect(screen.getByRole("button", { name: "More options" })).toHaveAttribute("aria-expanded", "false");
      fireEvent.click(within(strip).getByRole("button", { name: /^Hold timing/ }));
      await waitFor(() => expect(document.querySelector('[data-section="hold"]')).toHaveAttribute("data-found", "true"));
      expect(screen.getByRole("button", { name: "More options" })).toHaveAttribute("aria-expanded", "true");
      await waitFor(() => expect(screen.getByRole("heading", { name: "Hold timing", level: 2 })).toHaveFocus());
    });

    it("lists what was changed recently on Home, newest first", async () => {
      vi.spyOn(api, "saveSettings").mockImplementation(async (settings) => ({ ...structuredClone(browserState), settings: structuredClone(settings) }));
      render(<App />);
      await screen.findByRole("heading", { name: "Switchify PC" });
      expect(screen.getByText("Nothing changed yet.")).toBeInTheDocument();
      fireEvent.click(screen.getByRole("button", { name: "Settings" }));
      fireEvent.click(screen.getByRole("checkbox", { name: "Start with system" }));
      await waitFor(() => expect(screen.getByRole("checkbox", { name: "Start with system" })).toBeChecked());
      fireEvent.click(screen.getByRole("checkbox", { name: "Click when I stop" }));
      await waitFor(() => expect(screen.getByRole("checkbox", { name: "Click when I stop" })).toBeChecked());
      fireEvent.click(screen.getByRole("button", { name: "Home" }));
      const recent = screen.getByRole("heading", { name: "Recently changed", level: 2 }).closest("section")!;
      expect(within(recent).getAllByRole("button").map((button) => button.getAttribute("aria-label"))).toEqual([
        "Click when I stop, Settings, Input, On, after 1 s",
        "Start with system, Settings, General, On",
      ]);
      fireEvent.click(within(recent).getByRole("button", { name: /^Start with system/ }));
      expect(await screen.findByRole("heading", { name: "Settings", level: 1 })).toBeInTheDocument();
      await waitFor(() => expect(document.querySelector('[data-setting="startup"]')).toHaveAttribute("data-found", "true"));
    });

    it("links related settings on other pages under Also see", async () => {
      render(<App />);
      fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
      const also = screen.getByRole("heading", { name: "Also see", level: 2 }).closest("section")!;
      const names = within(also).getAllByRole("button").map((button) => button.getAttribute("aria-label")!);
      expect(names.length).toBeGreaterThan(0);
      for (const name of names) expect(name).not.toMatch(/, Mobile, /);
    });
  });

  describe("account on Settings", () => {
    const signedIn: AccountView = { available: true, signedIn: true, email: "me@example.com", pendingEmail: null, keychainUnavailable: false };
    const signedOut: AccountView = { ...signedIn, signedIn: false, email: null };
    let lastStatus: ReturnType<typeof vi.spyOn> | undefined;
    const mockAccount = (view: AccountView) => {
      let receive: (next: AccountView) => void = () => undefined;
      const status = vi.spyOn(api, "accountStatus").mockResolvedValue(view);
      vi.spyOn(api, "account").mockResolvedValue(view);
      vi.spyOn(api, "onAccount").mockImplementation(async (handler) => { receive = handler; return () => undefined; });
      lastStatus = status;
      vi.spyOn(api, "settingsSync").mockResolvedValue({ status: "upToDate", lastSyncedAt: null, message: null });
      vi.spyOn(api, "onSettingsSync").mockResolvedValue(() => undefined);
      return (next: AccountView) => receive(next);
    };

    it("shows the signed-in account as a section of Settings", async () => {
      mockAccount(signedIn);
      render(<App />);
      fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
      expect(screen.getByRole("heading", { name: "Account", level: 2 })).toBeInTheDocument();
      expect(await screen.findByRole("heading", { name: "me@example.com" })).toBeInTheDocument();
      expect(screen.queryByRole("heading", { name: "Account", level: 1 })).not.toBeInTheDocument();
      const strip = screen.getByRole("navigation", { name: "On this page" });
      await waitFor(() => expect(within(strip).getByRole("button", { name: /^Account/ })).toHaveAccessibleName("Account, Signed in as me@example.com. On"));
    });

    it("offers sign-in when signed out and follows account changes", async () => {
      const receive = mockAccount(signedOut);
      render(<App />);
      fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
      expect(await screen.findByRole("heading", { name: "Sign in to sync your settings" })).toBeInTheDocument();
      act(() => receive(signedIn));
      expect(await screen.findByRole("heading", { name: "me@example.com" })).toBeInTheDocument();
    });

    it("reads the account without the keychain-retrying command until the section is shown", async () => {
      mockAccount(signedIn);
      const retrying = vi.mocked(api.account);
      render(<App />);
      await screen.findByRole("heading", { name: "Switchify PC" });
      await waitFor(() => expect(lastStatus).toHaveBeenCalled());
      expect(retrying).not.toHaveBeenCalled();
    });

    it("stays neutral while the keychain is locked", async () => {
      mockAccount({ ...signedOut, keychainUnavailable: true });
      render(<App />);
      fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
      expect(await screen.findByText(/Your saved sign-in can't be read/)).toBeInTheDocument();
      expect(screen.queryByRole("heading", { name: "Sign in to sync your settings" })).not.toBeInTheDocument();
    });

    it("keeps an unsaved profile edit when navigation is cancelled", async () => {
      mockAccount(signedIn);
      vi.spyOn(window, "confirm").mockReturnValue(false);
      render(<App />);
      fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
      fireEvent.click(await screen.findByRole("button", { name: "New profile" }));
      fireEvent.change(screen.getByLabelText("Profile name"), { target: { value: "Unsaved" } });
      fireEvent.click(screen.getByRole("button", { name: "Settings" }));
      expect(window.confirm).toHaveBeenCalledWith("Discard unsaved profile changes?");
      expect(screen.getByLabelText("Profile name")).toHaveValue("Unsaved");
      expect(screen.queryByRole("heading", { name: "Settings", level: 1 })).not.toBeInTheDocument();
    });

    it("leaves the account out of builds without accounts", async () => {
      mockAccount({ ...signedOut, available: false });
      render(<App />);
      fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
      await screen.findByRole("region", { name: "About" });
      await waitFor(() => expect(screen.queryByRole("heading", { name: "Account", level: 2 })).not.toBeInTheDocument());
      const strip = screen.getByRole("navigation", { name: "On this page" });
      expect(within(strip).queryByRole("button", { name: /^Account/ })).not.toBeInTheDocument();
      expect(screen.getByRole("region", { name: "About" })).toHaveTextContent(`Switchify PC ${browserState.version}`);
    });
  });

  it("refreshes Switch Forwarding profiles when settings sync changes them", async () => {
    let changed: () => void = () => undefined;
    vi.spyOn(api, "onProfilesChanged").mockImplementation(async (handler) => { changed = handler; return () => undefined; });
    const list = vi.spyOn(api, "listProfiles");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
    await screen.findByRole("button", { name: "New profile" });
    const before = list.mock.calls.length;
    list.mockResolvedValueOnce([{ id: "builtin.keyboard", version: 1, name: "Generic keyboard", provider: "mapped", builtIn: true, bindings: [] },
      { id: "6f1c1a52-6a0e-4c2b-9d0f-2f8a7b9e4c11", version: 2, name: "Synced desk", provider: "mapped", builtIn: false, bindings: [] }]);
    act(() => changed());
    await waitFor(() => expect(list.mock.calls.length).toBe(before + 1));
    expect(await screen.findByRole("button", { name: /Synced desk/ })).toBeInTheDocument();
  });

  it("closes pristine new and duplicated profiles without a discard prompt", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
    const newProfile = await screen.findByRole("button", { name: "New profile" });
    fireEvent.click(newProfile);
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    await waitFor(() => expect(newProfile).toHaveFocus());

    const builtIn = screen.getByRole("button", { name: /Generic keyboard.*Built in/ });
    fireEvent.click(builtIn);
    fireEvent.click(screen.getByRole("button", { name: "Duplicate" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  });

  it("prevents navigation from discarding a modified profile", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
    fireEvent.click(await screen.findByRole("button", { name: "New profile" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Profile name" }), { target: { value: "Modified controls" } });

    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    expect(confirm).toHaveBeenCalledWith("Discard unsaved profile changes?");
    expect(screen.getByRole("dialog", { name: "Edit switch profile" })).toBeInTheDocument();

    confirm.mockReturnValue(true);
    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    expect(await screen.findByRole("heading", { name: "Switchify PC" })).toBeInTheDocument();
  });

  it("coordinates native close and quit with the dirty editor", async () => {
    let exitHandler: ((action: "hide" | "quit") => void) | undefined;
    const cancelExit = vi.spyOn(api, "cancelProfileExit").mockResolvedValue();
    const completeExit = vi.spyOn(api, "completeProfileExit").mockResolvedValue();
    vi.spyOn(api, "onProfileExitRequested").mockImplementation(async (handler) => {
      exitHandler = handler;
      return () => undefined;
    });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
    fireEvent.click(await screen.findByRole("button", { name: "New profile" }));
    fireEvent.change(screen.getByRole("textbox", { name: "Profile name" }), { target: { value: "Unsaved controls" } });

    act(() => exitHandler?.("hide"));
    expect(await screen.findByText("The window will close and your profile changes will be lost.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Keep editing" }));
    expect(cancelExit).toHaveBeenCalledOnce();
    expect(screen.getByRole("dialog", { name: "Edit switch profile" })).toBeInTheDocument();

    act(() => exitHandler?.("quit"));
    expect(await screen.findByText("Switchify PC will quit and your profile changes will be lost.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Discard and quit" }));
    expect(completeExit).toHaveBeenCalledOnce();
    expect(screen.queryByRole("dialog", { name: "Edit switch profile" })).not.toBeInTheDocument();
  });

  it("confirms deletion and moves focus when the profile row is removed", async () => {
    const custom: SwitchProfile = {
      id: crypto.randomUUID(), version: 1, name: "Scanning controls", provider: "mapped", builtIn: false,
      bindings: Array.from({ length: 8 }, (_, index) => ({ switchId: index + 1, type: "none" as const })),
    };
    vi.spyOn(api, "listProfiles").mockResolvedValue([custom]);
    vi.spyOn(api, "deleteProfile").mockResolvedValue([]);
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
    fireEvent.click(await screen.findByRole("button", { name: /Scanning controls.*Custom/ }));
    fireEvent.change(screen.getByRole("textbox", { name: "Profile name" }), { target: { value: "Changed controls" } });
    fireEvent.click(screen.getByRole("button", { name: "Delete" }));

    const confirmation = screen.getByRole("alertdialog", { name: "Delete Scanning controls?" });
    expect(within(confirmation).getByRole("button", { name: "Keep editing" })).toHaveFocus();
    fireEvent.click(within(confirmation).getByRole("button", { name: "Delete profile" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "New profile" })).toHaveFocus());
  });

  it("keeps the editor open and focused when saving fails", async () => {
    vi.spyOn(api, "saveProfile").mockRejectedValue(new Error("Profile storage unavailable"));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
    fireEvent.click(await screen.findByRole("button", { name: "New profile" }));
    fireEvent.click(screen.getByRole("button", { name: "Save profile" }));

    expect(await screen.findByRole("alert", { name: "" })).toHaveTextContent("Profile storage unavailable");
    expect(screen.getByRole("dialog", { name: "Edit switch profile" })).toHaveFocus();
  });

  it("exposes setup status and troubleshooting actions", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Help" }));
    expect(screen.getByRole("heading", { name: "Local switches" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Export" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Application update" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "View updates" }));
    const updates = await screen.findByRole("region", { name: "Updates" });
    await waitFor(() => expect(updates).toHaveFocus());
  });

  it("guides macOS users through required and stale accessibility entries", async () => {
    const originalPlatform = browserState.capabilities.platform;
    browserState.capabilities.platform = "macos";
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Help" }));
    // Setup and Fix a problem both carry the guidance, so it is found twice.
    expect(screen.getAllByText(/Enable “Switchify PC” in Accessibility/)).toHaveLength(2);
    expect(screen.getAllByText(/select the stale row, click Remove/)).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: "Open Accessibility Settings" })).toHaveLength(2);
    browserState.capabilities.platform = originalPlatform;
  });

  it("updates accessibility to Ready from the runtime event", async () => {
    let stateHandler: ((state: typeof browserState) => void) | undefined;
    const listener = vi.spyOn(api, "onState").mockImplementation(async (handler) => {
      stateHandler = handler;
      return () => undefined;
    });
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    stateHandler?.({ ...structuredClone(browserState), accessibility: "granted" });
    expect(await screen.findByText("Ready")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open Accessibility Settings" })).not.toBeInTheDocument();
    listener.mockRestore();
  });

  it("lists the Help sections on the page strip and jumps to one", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Help" }));

    const strip = screen.getByRole("navigation", { name: "On this page" });
    expect(within(strip).getAllByRole("button").map((button) => button.getAttribute("aria-label")!.split(",")[0])).toEqual(["Setup", "Fix a problem"]);
    // Both sections are on the page at once.
    expect(screen.getByRole("heading", { name: "Local switches" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Export" })).toBeInTheDocument();

    fireEvent.click(within(strip).getByRole("button", { name: /^Fix a problem/ }));
    const section = document.querySelector('[data-section="fix"]')!;
    await waitFor(() => expect(section).toHaveAttribute("data-found", "true"));
    expect(screen.getByRole("heading", { name: "Fix a problem", level: 2 })).toHaveFocus();
  });
});
