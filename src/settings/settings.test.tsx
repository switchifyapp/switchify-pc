import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../App";
import { api, browserState } from "../api";
import type { AppSettings } from "../types";

const defaultBrowserSettings = structuredClone(browserState.settings);
const defaultCapabilities = structuredClone(browserState.capabilities);

function stateWithSettings(settings: AppSettings) {
  return { ...structuredClone(browserState), settings: structuredClone(settings) };
}

const selectTab = (name: string) => fireEvent.click(screen.getByRole("tab", { name }));

describe("Switchify PC settings", () => {
  beforeEach(() => {
    browserState.settings = structuredClone(defaultBrowserSettings);
    browserState.capabilities = structuredClone(defaultCapabilities);
    browserState.bluetooth = "initializing";
    browserState.pendingPairings = [];
    browserState.pairedDevices = [];
    browserState.connectedDeviceName = null;
    browserState.lastActivity = null;
    browserState.diagnostics = { recentBluetooth: [], lastDisconnect: null, recentErrors: [] };
    browserState.telemetry = { consent: "undecided", available: true };
    browserState.updater = { status: "unconfigured", version: null, downloadedBytes: 0, totalBytes: null, error: null, retryAction: null };
    browserState.setup = { shown: true, completed: false, autoOpenEligible: false };
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("shows update progress and exposes cancellation in Settings", async () => {
    browserState.updater = { status: "downloading", version: "1.0.0-beta.2", downloadedBytes: 50, totalBytes: 200, error: null, retryAction: null };
    const cancel = vi.spyOn(api, "cancelUpdateDownload").mockResolvedValue(structuredClone(browserState));
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    selectTab("Updates");

    expect(screen.getByText("Downloading Switchify PC 1.0.0-beta.2…")).toBeInTheDocument();
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
    selectTab("Updates");
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
    selectTab("Updates");
    fireEvent.click(screen.getByRole("button", { name: "Install and restart" }));
    expect(install).toHaveBeenCalledOnce();
  });

  it("retries a cancelled download from the beginning", async () => {
    browserState.updater = { status: "cancelled", version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error: null, retryAction: "download" };
    const download = vi.spyOn(api, "downloadUpdate").mockResolvedValue(structuredClone(browserState));
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    selectTab("Updates");
    expect(screen.getByRole("status")).toHaveTextContent("Download cancelled. You can retry when ready.");
    fireEvent.click(screen.getByRole("button", { name: "Retry download" }));
    expect(download).toHaveBeenCalledOnce();
  });

  it("reports manual update-check failures in Settings", async () => {
    const checkForUpdates = vi.spyOn(api, "checkForUpdates").mockRejectedValue(new Error("Update service unavailable"));
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    selectTab("Updates");
    fireEvent.click(screen.getByRole("button", { name: "Check for updates" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Update service unavailable");
    checkForUpdates.mockRestore();
  });

  it.each([
    ["checking", "Checking for updates…", "Checking"],
    ["current", "Switchify PC is up to date.", "Check for updates"],
  ] as const)("keeps the %s update state visible in Settings", async (status, description, action) => {
    browserState.updater = { status, version: null, downloadedBytes: 0, totalBytes: null, error: null, retryAction: null };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    selectTab("Updates");

    expect(screen.getByText(description)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: action })).toBeInTheDocument();
  });

  it("exposes key repeat settings and disables them with the toggle", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    selectTab("Pointer");

    const toggle = screen.getByRole("checkbox", { name: "Repeat held keys" });
    expect(toggle).toBeChecked();

    const delay = screen.getByRole("group", { name: "Delay before repeating" });
    expect(delay).not.toBeDisabled();
    for (const label of ["None", "Short", "Medium", "Long"]) {
      expect(within(delay).getByRole("button", { name: label })).toBeInTheDocument();
    }
    expect(within(delay).getByRole("button", { name: "Medium" })).toHaveAttribute("aria-pressed", "true");

    const interval = screen.getByRole("group", { name: "Key interval" });
    expect(within(interval).getByRole("button", { name: "0.25s" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(within(interval).getByRole("button", { name: "0.5s" }));
    expect(within(interval).getByRole("button", { name: "0.5s" })).toHaveAttribute("aria-pressed", "true");

    fireEvent.click(within(delay).getByRole("button", { name: "None" }));
    expect(within(delay).getByRole("button", { name: "None" })).toHaveAttribute("aria-pressed", "true");

    // Turning the feature off must disable its cadence controls, matching the
    // mouse repeat and dwell blocks.
    fireEvent.click(toggle);
    expect(screen.getByRole("group", { name: "Delay before repeating" })).toBeDisabled();
    expect(screen.getByRole("group", { name: "Key interval" })).toBeDisabled();
  });

  it("opens settings with accessible General and Pointer controls", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Save settings" })).not.toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Start with system" })).toBeInTheDocument();

    selectTab("Pointer");
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
  });

  it("exposes the dwell controls and their explanatory note", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    selectTab("Pointer");

    const dwell = screen.getByRole("checkbox", { name: "Dwell to click" });
    expect(dwell).not.toBeChecked();
    expect(screen.getByRole("group", { name: "Dwell delay" })).toBeDisabled();
    fireEvent.click(dwell);
    const dwellDelay = screen.getByRole("group", { name: "Dwell delay" });
    expect(dwellDelay).not.toBeDisabled();
    expect(within(dwellDelay).getAllByRole("button")).toHaveLength(10);
    for (const label of ["0.5s", "1s", "1.5s", "2s", "3s", "4s", "5s", "6s", "7s", "8s"]) {
      expect(within(dwellDelay).getByRole("button", { name: label })).toBeInTheDocument();
    }
    expect(within(dwellDelay).getByRole("button", { name: "1s" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(within(dwellDelay).getByRole("button", { name: "1.5s" }));
    expect(within(dwellDelay).getByRole("button", { name: "1.5s" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText(/After Android pointer movement stops/)).toBeInTheDocument();
  });

  it("exposes the cursor overlay controls on their own tab", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    selectTab("Cursor");

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
    selectTab("Privacy");

    fireEvent.click(screen.getByRole("checkbox", { name: "Share anonymous diagnostic data" }));

    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ shareDiagnostics: true })));
    expect(screen.getByRole("checkbox", { name: "Share anonymous diagnostic data" })).toBeChecked();
  });

  it("persists dwell enablement and delay through automatic settings saves", async () => {
    const saveSettings = vi.spyOn(api, "saveSettings").mockImplementation(async (settings) => stateWithSettings(settings));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    selectTab("Pointer");

    fireEvent.click(screen.getByRole("checkbox", { name: "Dwell to click" }));
    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ dwellClickEnabled: true, dwellClickDelayMs: 1000 })));
    const dwellDelay = screen.getByRole("group", { name: "Dwell delay" });
    fireEvent.click(within(dwellDelay).getByRole("button", { name: "8s" }));
    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ dwellClickEnabled: true, dwellClickDelayMs: 8000 })));
    expect(within(dwellDelay).getByRole("button", { name: "8s" })).toHaveAttribute("aria-pressed", "true");
  });

  it("restores confirmed dwell settings when automatic saving fails", async () => {
    vi.spyOn(api, "saveSettings").mockRejectedValueOnce(new Error("Settings storage unavailable"));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    selectTab("Pointer");

    fireEvent.click(screen.getByRole("checkbox", { name: "Dwell to click" }));

    expect(await screen.findByRole("alert")).toHaveTextContent("Settings storage unavailable");
    expect(screen.getByRole("checkbox", { name: "Dwell to click" })).not.toBeChecked();
    expect(screen.getByRole("group", { name: "Dwell delay" })).toBeDisabled();
  });

  it("explains telemetry consent and links to the privacy policy", async () => {
    const first = render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    selectTab("Privacy");
    expect(screen.getByText(/Nothing is sent unless you choose Share diagnostics/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Don't share" }));
    expect(await screen.findByText("Opted out. No diagnostic reports are stored or sent.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Privacy policy" })).toHaveAttribute("href", "https://switchifyapp.com/privacy");

    first.unmount();
    browserState.telemetry = { consent: "undecided", available: false };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    selectTab("Privacy");
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
    selectTab("Pointer");

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
    selectTab("Pointer");

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

    selectTab("Privacy");
    fireEvent.click(screen.getByRole("checkbox", { name: "Share anonymous diagnostic data" }));
    selectTab("Pointer");
    fireEvent.click(screen.getByRole("button", { name: "50% pointer speed" }));
    act(() => stateHandler?.(stateWithSettings({ ...defaultBrowserSettings, pointerScalePercent: 150 })));

    expect(screen.getByRole("combobox", { name: "Exact pointer speed" })).toHaveValue("150");
    selectTab("Privacy");
    expect(screen.getByRole("checkbox", { name: "Share anonymous diagnostic data" })).toBeChecked();

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

  it("focuses Updates when the update banner is selected from Settings", async () => {
    browserState.updater = { status: "available", version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error: null, retryAction: null };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.queryByRole("region", { name: "Updates" })).not.toBeInTheDocument();
    fireEvent.click(within(screen.getByRole("status", { name: "Application update" })).getByRole("button", { name: "View update" }));
    const updates = await screen.findByRole("region", { name: "Updates" });
    await waitFor(() => expect(updates).toHaveFocus());
    expect(screen.getByRole("tab", { name: "Updates" })).toHaveAttribute("aria-selected", "true");
  });

  it("does not replay a consumed Updates focus request on normal Settings navigation", async () => {
    browserState.updater = { status: "available", version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error: null, retryAction: null };
    render(<App />);
    const banner = await screen.findByRole("status", { name: "Application update" });
    fireEvent.click(within(banner).getByRole("button", { name: "View update" }));
    const updates = await screen.findByRole("region", { name: "Updates" });
    await waitFor(() => expect(updates).toHaveFocus());

    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("tab", { name: "General" })).toHaveAttribute("aria-selected", "true");
    expect(screen.queryByRole("region", { name: "Updates" })).not.toBeInTheDocument();
  });


  it("names every settings section as a tab and marks only the active one selected", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    const tablist = screen.getByRole("tablist", { name: "Settings sections" });
    expect(within(tablist).getAllByRole("tab").map((tab) => tab.textContent))
      .toEqual(["General", "Pointer", "Cursor", "Privacy", "Updates"]);
    expect(screen.getByRole("tab", { name: "General" })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel")).toHaveAccessibleName("General");
  });

  it("keeps a single tab stop and moves focus with the arrow keys without activating", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    const general = screen.getByRole("tab", { name: "General" });
    const pointer = screen.getByRole("tab", { name: "Pointer" });
    const updates = screen.getByRole("tab", { name: "Updates" });
    expect(general).toHaveAttribute("tabindex", "0");
    expect(pointer).toHaveAttribute("tabindex", "-1");

    general.focus();
    fireEvent.keyDown(general, { key: "ArrowRight" });
    expect(pointer).toHaveFocus();
    // Manual activation: moving focus must not change the selected panel.
    expect(general).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("checkbox", { name: "Start with system" })).toBeInTheDocument();
    // The tab stop follows focus, so tabbing out and back does not discard it.
    expect(pointer).toHaveAttribute("tabindex", "0");
    expect(general).toHaveAttribute("tabindex", "-1");

    fireEvent.keyDown(pointer, { key: "ArrowLeft" });
    expect(general).toHaveFocus();
    fireEvent.keyDown(general, { key: "ArrowLeft" });
    expect(updates).toHaveFocus();
    fireEvent.keyDown(updates, { key: "ArrowRight" });
    expect(general).toHaveFocus();
    fireEvent.keyDown(general, { key: "End" });
    expect(updates).toHaveFocus();
    fireEvent.keyDown(updates, { key: "Home" });
    expect(general).toHaveFocus();

    fireEvent.click(pointer);
    expect(pointer).toHaveAttribute("aria-selected", "true");
    expect(pointer).toHaveAttribute("tabindex", "0");
    expect(general).toHaveAttribute("tabindex", "-1");
  });

  it("omits the Cursor tab when the platform cannot show a cursor overlay", async () => {
    browserState.capabilities = { ...structuredClone(defaultCapabilities), cursorOverlay: false };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    expect(screen.queryByRole("tab", { name: "Cursor" })).not.toBeInTheDocument();
    expect(screen.getAllByRole("tab").map((tab) => tab.textContent))
      .toEqual(["General", "Pointer", "Privacy", "Updates"]);
    selectTab("Pointer");
    expect(screen.queryByRole("checkbox", { name: "Show cursor overlay" })).not.toBeInTheDocument();
  });

  it("switches to the Updates tab when the banner is used from another tab", async () => {
    browserState.updater = { status: "available", version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error: null, retryAction: null };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    selectTab("Pointer");
    expect(screen.getByRole("tab", { name: "Pointer" })).toHaveAttribute("aria-selected", "true");

    fireEvent.click(within(screen.getByRole("status", { name: "Application update" })).getByRole("button", { name: "View update" }));

    expect(await screen.findByRole("region", { name: "Updates" })).toHaveFocus();
    expect(screen.getByRole("tab", { name: "Updates" })).toHaveAttribute("aria-selected", "true");
  });

  it("keeps a pending edit on an inactive tab and saves it", async () => {
    const saveSettings = vi.spyOn(api, "saveSettings").mockImplementation(async (settings) => stateWithSettings(settings));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    selectTab("Pointer");
    fireEvent.click(screen.getByRole("button", { name: "50% pointer speed" }));
    selectTab("Privacy");
    selectTab("Pointer");

    expect(screen.getByRole("button", { name: "50% pointer speed" })).toHaveAttribute("aria-pressed", "true");
    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ pointerScalePercent: 50 })));
  });


  it("keeps the tab stop on the last focused tab after focus leaves the tablist", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    const general = screen.getByRole("tab", { name: "General" });
    const pointer = screen.getByRole("tab", { name: "Pointer" });
    general.focus();
    fireEvent.keyDown(general, { key: "ArrowRight" });
    expect(pointer).toHaveAttribute("tabindex", "0");

    const startup = screen.getByRole("checkbox", { name: "Start with system" });
    startup.focus();
    expect(pointer).toHaveAttribute("tabindex", "0");
    expect(general).toHaveAttribute("tabindex", "-1");
  });

  it("makes the panel a tab stop only when nothing inside it can take focus", async () => {
    browserState.updater = { status: "checking", version: null, downloadedBytes: 0, totalBytes: null, error: null, retryAction: null };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    // General holds a focusable toggle, so the panel must not add its own stop.
    expect(screen.getByRole("tabpanel")).not.toHaveAttribute("tabindex");

    selectTab("Updates");
    // While checking, the only button is disabled, so the panel becomes reachable.
    await waitFor(() => expect(screen.getByRole("tabpanel")).toHaveAttribute("tabindex", "0"));
  });

  it("points aria-controls only at the panel that is rendered", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    const general = screen.getByRole("tab", { name: "General" });
    expect(general).toHaveAttribute("aria-controls", "settings-panel-general");
    expect(document.getElementById("settings-panel-general")).toBeInTheDocument();
    for (const name of ["Pointer", "Cursor", "Privacy", "Updates"]) {
      expect(screen.getByRole("tab", { name })).not.toHaveAttribute("aria-controls");
    }
    // The panel is not a tab stop of its own; its controls are.
    expect(screen.getByRole("tabpanel")).not.toHaveAttribute("tabindex");
  });

});
