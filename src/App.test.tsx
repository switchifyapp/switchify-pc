import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { api, browserState } from "./api";
import type { AppSettings, SwitchProfile } from "./types";

const defaultBrowserSettings = structuredClone(browserState.settings);

function stateWithSettings(settings: AppSettings) {
  return { ...structuredClone(browserState), settings: structuredClone(settings) };
}

describe("Switchify PC shell", () => {
  beforeEach(() => {
    browserState.settings = structuredClone(defaultBrowserSettings);
    browserState.pendingPairings = [];
    browserState.pairedDevices = [];
    browserState.connectedDeviceName = null;
    browserState.diagnostics = { recentBluetooth: [], lastDisconnect: null, recentErrors: [] };
    browserState.telemetry = { consent: "undecided", available: true };
    browserState.updater = { status: "unconfigured", version: null, downloadedBytes: 0, totalBytes: null, error: null, retryAction: null };
    browserState.setup = { shown: true, completed: false, autoOpenEligible: false };
  });

  afterEach(() => {
    vi.restoreAllMocks();
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

  it("shows the version instead of the OS and checks for updates from the footer", async () => {
    let finishUpdate: ((state: typeof browserState) => void) | undefined;
    const checkForUpdates = vi.spyOn(api, "checkForUpdates").mockImplementation(() => new Promise((resolve) => { finishUpdate = resolve; }));
    const { container } = render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });

    const footer = container.querySelector(".sidebar-footer");
    expect(footer).toHaveTextContent(`v${browserState.version}`);
    expect(footer).not.toHaveTextContent(browserState.capabilities.platform === "macos" ? "macOS" : "Windows");

    const button = screen.getByRole("button", { name: "Check for updates" });
    fireEvent.click(button);
    expect(checkForUpdates).toHaveBeenCalledOnce();
    expect(button).toBeDisabled();
    expect(button.querySelector("svg")).toHaveClass("spin");

    finishUpdate?.(structuredClone(browserState));
    await waitFor(() => expect(button).not.toBeDisabled());
    expect(button.querySelector("svg")).not.toHaveClass("spin");
    checkForUpdates.mockRestore();
  });

  it("shows update progress and exposes cancellation in Settings", async () => {
    browserState.updater = { status: "downloading", version: "1.0.0-beta.2", downloadedBytes: 50, totalBytes: 200, error: null, retryAction: null };
    const cancel = vi.spyOn(api, "cancelUpdateDownload").mockResolvedValue(structuredClone(browserState));
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));

    expect(screen.getByRole("status")).toHaveTextContent("Downloading Switchify PC 1.0.0-beta.2");
    expect(screen.getByRole("progressbar", { name: "Update download progress" })).toHaveAttribute("value", "50");
    expect(document.querySelector(".update-controls > span")).toHaveTextContent("25%");
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(cancel).toHaveBeenCalledOnce();
  });

  it("offers the correct retry action after a failure", async () => {
    browserState.updater = { status: "failed", version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error: "Download failed", retryAction: "download" };
    const download = vi.spyOn(api, "downloadUpdate").mockResolvedValue(structuredClone(browserState));
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("alert")).toHaveTextContent("Download failed");
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    expect(download).toHaveBeenCalledOnce();
  });

  it("offers installation and restart when a download is ready", async () => {
    browserState.updater = { status: "readyToInstall", version: "1.0.0-beta.2", downloadedBytes: 200, totalBytes: 200, error: null, retryAction: null };
    const install = vi.spyOn(api, "installUpdate").mockResolvedValue(structuredClone(browserState));
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    fireEvent.click(screen.getByRole("button", { name: "Install and restart" }));
    expect(install).toHaveBeenCalledOnce();
  });

  it("retries a cancelled download from the beginning", async () => {
    browserState.updater = { status: "cancelled", version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error: null, retryAction: "download" };
    const download = vi.spyOn(api, "downloadUpdate").mockResolvedValue(structuredClone(browserState));
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("status")).toHaveTextContent("Download cancelled. You can retry when ready.");
    fireEvent.click(screen.getByRole("button", { name: "Retry download" }));
    expect(download).toHaveBeenCalledOnce();
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

  it("reports update-check failures from the footer", async () => {
    const checkForUpdates = vi.spyOn(api, "checkForUpdates").mockRejectedValue(new Error("Update service unavailable"));
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Update service unavailable");
    checkForUpdates.mockRestore();
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
    const guide = await screen.findByRole("dialog", { name: "Bluetooth and input access" });
    await waitFor(() => expect(guide).toHaveFocus());
    expect(screen.getByLabelText("Step 1 of 5")).toBeInTheDocument();
    await waitFor(() => expect(markShown).toHaveBeenCalledOnce());
    fireEvent.click(screen.getByRole("button", { name: "Skip for now" }));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Bluetooth and input access" })).not.toBeInTheDocument());
    expect(markShown).toHaveBeenCalledTimes(2);
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

  it("reopens setup from Support with the Android download and explicit choices", async () => {
    browserState.pairedDevices = [{ deviceId: "phone-1", deviceName: "Pixel", pairedAt: 1, lastSeenAt: null }];
    const complete = vi.spyOn(api, "completeSetup").mockImplementation(async (startWithSystem, shareDiagnostics) => ({
      ...structuredClone(browserState),
      settings: { ...structuredClone(browserState.settings), startWithSystem, shareDiagnostics },
      telemetry: { ...browserState.telemetry, consent: shareDiagnostics ? "enabled" : "disabled" },
      setup: { shown: true, completed: true, autoOpenEligible: false },
    }));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Support" }));
    fireEvent.click(screen.getByRole("button", { name: "Open setup guide" }));
    fireEvent.click(await screen.findByRole("button", { name: "Next" }));
    expect(screen.getByRole("link", { name: "Open Google Play" })).toHaveAttribute("href", "https://play.google.com/store/apps/details?id=com.enaboapps.switchify");
    expect(screen.getByRole("img", { name: "QR code for Switchify on Google Play" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByRole("button", { name: "Next" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Start manually" }));
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByRole("button", { name: "Finish" })).toBeDisabled();
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
    fireEvent.click(await screen.findByRole("button", { name: "Support" }));
    fireEvent.click(screen.getByRole("tab", { name: "Troubleshooting" }));
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
    expect(screen.getByRole("button", { name: "Reject pairing request from Pixel, code 111111" })).toHaveFocus();
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

    render(<App />);
    const setup = await screen.findByRole("dialog", { name: "Bluetooth and input access" });
    fireEvent.click(within(setup).getByRole("button", { name: "Next" }));
    fireEvent.click(within(setup).getByRole("button", { name: "Next" }));
    expect(screen.getByLabelText("Verification code for Galaxy")).toHaveTextContent("063781");

    act(() => stateHandler?.({
      ...structuredClone(browserState),
      pendingPairings: [],
      lastActivity: { kind: "info", message: "Pairing request cancelled." },
    }));

    await waitFor(() => expect(screen.queryByLabelText("Verification code for Galaxy")).not.toBeInTheDocument());
    expect(screen.getByRole("heading", { name: "Waiting for an Android device" })).toBeInTheDocument();
  });

  it("opens settings with accessible native controls", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Save settings" })).not.toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Start with system" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "100% pointer speed" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("checkbox", { name: "Repeat mouse movement" })).toBeChecked();
    expect(screen.getByRole("group", { name: "Movement acceleration" })).not.toBeDisabled();
    expect(screen.getAllByRole("button", { name: "Medium" })[0]).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: "50% pointer speed" }));
    expect(screen.getByRole("button", { name: "50% pointer speed" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText("2.5")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("combobox", { name: "Exact pointer speed" }), { target: { value: "125" } });
    expect(screen.getByRole("combobox", { name: "Exact pointer speed" })).toHaveValue("125");
    expect(screen.getByText("5.5")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("checkbox", { name: "Repeat mouse movement" }));
    expect(screen.getByRole("group", { name: "Movement acceleration" })).toBeDisabled();
    expect(screen.getByRole("checkbox", { name: "Show cursor overlay" })).toBeChecked();
    expect(screen.getByRole("button", { name: "While controlling" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText(/On input hides shortly/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "On input" }));
    expect(screen.getByRole("button", { name: "On input" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getAllByRole("button", { name: "Medium" }).at(-1)).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("radio", { name: "Red" })).toBeChecked();
    fireEvent.click(screen.getByRole("checkbox", { name: "Show crosshairs" }));
    expect(screen.getByRole("checkbox", { name: "Show crosshairs" })).toBeChecked();
    fireEvent.click(screen.getByRole("checkbox", { name: "Show cursor overlay" }));
    expect(screen.getByRole("checkbox", { name: "Show crosshairs" })).toBeDisabled();
  });

  it("automatically saves a settings change", async () => {
    const saveSettings = vi.spyOn(api, "saveSettings").mockImplementation(async (settings) => stateWithSettings(settings));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    fireEvent.click(screen.getByRole("checkbox", { name: "Share anonymous diagnostic data" }));

    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ shareDiagnostics: true })));
    expect(screen.getByRole("checkbox", { name: "Share anonymous diagnostic data" })).toBeChecked();
  });

  it("explains telemetry consent and links to the privacy policy", async () => {
    const first = render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.getByText(/Nothing is sent unless you choose Share diagnostics/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Don't share" }));
    expect(await screen.findByText("Opted out. No diagnostic reports are stored or sent.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Privacy policy" })).toHaveAttribute("href", "https://switchifyapp.com/privacy");

    first.unmount();
    browserState.telemetry = { consent: "undecided", available: false };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(await screen.findByRole("checkbox", { name: "Share anonymous diagnostic data" })).toBeDisabled();
    expect(screen.getByText("Diagnostic reporting is unavailable in this build.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Share diagnostics" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Don't share" })).toBeEnabled();
  });

  it("serializes rapid settings changes without applying a stale response", async () => {
    const saves: Array<{ settings: AppSettings; resolve: (state: typeof browserState) => void }> = [];
    vi.spyOn(api, "saveSettings").mockImplementation((settings) => new Promise((resolve) => {
      saves.push({ settings: structuredClone(settings), resolve });
    }));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    fireEvent.click(screen.getByRole("button", { name: "50% pointer speed" }));
    fireEvent.click(screen.getByRole("button", { name: "75% pointer speed" }));

    expect(saves).toHaveLength(1);
    expect(screen.getByRole("button", { name: "75% pointer speed" })).toHaveAttribute("aria-pressed", "true");

    await act(async () => {
      saves[0].resolve(stateWithSettings(saves[0].settings));
    });
    await waitFor(() => expect(saves).toHaveLength(2));
    expect(saves[1].settings.pointerScalePercent).toBe(75);
    expect(screen.getByRole("button", { name: "75% pointer speed" })).toHaveAttribute("aria-pressed", "true");

    await act(async () => {
      saves[1].resolve(stateWithSettings(saves[1].settings));
    });
  });

  it("preserves newer runtime state when a settings save completes", async () => {
    let stateHandler: ((state: typeof browserState) => void) | undefined;
    let finishSave: ((state: typeof browserState) => void) | undefined;
    vi.spyOn(api, "onState").mockImplementation(async (handler) => {
      stateHandler = handler;
      return () => undefined;
    });
    vi.spyOn(api, "saveSettings").mockImplementation(() => new Promise((resolve) => {
      finishSave = resolve;
    }));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    fireEvent.click(screen.getByRole("button", { name: "50% pointer speed" }));
    const runtimeState = {
      ...structuredClone(browserState),
      bluetooth: "connected" as const,
      connectedDeviceName: "Newer connected device",
    };
    act(() => stateHandler?.(runtimeState));
    await act(async () => {
      finishSave?.(stateWithSettings({ ...defaultBrowserSettings, pointerScalePercent: 50 }));
    });

    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    expect(screen.getByText("Newer connected device")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Disconnect" })).toBeInTheDocument();
  });

  it("rebases local edits on newer backend settings", async () => {
    let stateHandler: ((state: typeof browserState) => void) | undefined;
    const saves: Array<{ settings: AppSettings; resolve: (state: typeof browserState) => void }> = [];
    vi.spyOn(api, "onState").mockImplementation(async (handler) => {
      stateHandler = handler;
      return () => undefined;
    });
    vi.spyOn(api, "saveSettings").mockImplementation((settings) => new Promise((resolve) => {
      saves.push({ settings: structuredClone(settings), resolve });
    }));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    fireEvent.click(screen.getByRole("checkbox", { name: "Share anonymous diagnostic data" }));
    fireEvent.click(screen.getByRole("button", { name: "50% pointer speed" }));
    act(() => stateHandler?.(stateWithSettings({ ...defaultBrowserSettings, pointerScalePercent: 150 })));

    expect(screen.getByRole("checkbox", { name: "Share anonymous diagnostic data" })).toBeChecked();
    expect(screen.getByRole("combobox", { name: "Exact pointer speed" })).toHaveValue("150");

    await act(async () => {
      saves[0].resolve(stateWithSettings(saves[0].settings));
    });
    await waitFor(() => expect(saves).toHaveLength(2));
    expect(saves[1].settings).toEqual(expect.objectContaining({
      pointerScalePercent: 150,
      shareDiagnostics: true,
    }));

    await act(async () => {
      saves[1].resolve(stateWithSettings(saves[1].settings));
    });
  });

  it("restores confirmed settings when automatic saving fails", async () => {
    vi.spyOn(api, "saveSettings").mockRejectedValueOnce(new Error("Settings storage unavailable"));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    const startup = screen.getByRole("checkbox", { name: "Start with system" });
    fireEvent.click(startup);
    expect(startup).toBeChecked();

    expect(await screen.findByRole("alert")).toHaveTextContent("Settings storage unavailable");
    expect(startup).not.toBeChecked();
  });

  it("creates a profile and records a desired key", async () => {
    let saved: SwitchProfile | undefined;
    vi.spyOn(api, "saveProfile").mockImplementation(async (profile) => {
      saved = structuredClone(profile);
      return [profile];
    });
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Switch control" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Switch control" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Switch control" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Switch control" }));
    const opener = await screen.findByRole("button", { name: "New profile" });
    fireEvent.click(opener);
    fireEvent.change(screen.getByRole("textbox", { name: "Profile name" }), { target: { value: "Unsaved controls" } });
    fireEvent.click(screen.getByTitle("Close"));

    const confirmation = screen.getByRole("alertdialog", { name: "Discard unsaved changes?" });
    expect(within(confirmation).getByRole("button", { name: "Keep editing" })).toHaveFocus();
    fireEvent.click(within(confirmation).getByRole("button", { name: "Discard changes" }));
    await waitFor(() => expect(opener).toHaveFocus());
  });

  it("closes pristine new and duplicated profiles without a discard prompt", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Switch control" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Switch control" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Switch control" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Switch control" }));
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
    fireEvent.click(await screen.findByRole("button", { name: "Switch control" }));
    fireEvent.click(await screen.findByRole("button", { name: "New profile" }));
    fireEvent.click(screen.getByRole("button", { name: "Save profile" }));

    expect(await screen.findByRole("alert", { name: "" })).toHaveTextContent("Profile storage unavailable");
    expect(screen.getByRole("dialog", { name: "Edit switch profile" })).toHaveFocus();
  });

  it("exposes setup status and troubleshooting actions", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Support" }));
    expect(screen.getByRole("heading", { name: "Android device" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("tab", { name: "Troubleshooting" }));
    expect(screen.getByRole("button", { name: "Export" })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Application update" })).toBeInTheDocument();
  });

  it("guides macOS users through required and stale accessibility entries", async () => {
    const originalPlatform = browserState.capabilities.platform;
    browserState.capabilities.platform = "macos";
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Support" }));
    expect(screen.getByText(/Enable “Switchify PC” in Accessibility/)).toBeInTheDocument();
    expect(screen.getByText(/select the stale row, click Remove/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Accessibility Settings" })).toBeInTheDocument();
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
});
