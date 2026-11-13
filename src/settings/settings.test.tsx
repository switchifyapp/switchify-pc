import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { StrictMode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "../App";
import { api, browserState } from "../api";
import type { AppSettings } from "../types";
import { speedLevel, speedPercent } from "./pointerSpeedScale";

const defaultBrowserSettings = structuredClone(browserState.settings);
const defaultCapabilities = structuredClone(browserState.capabilities);

function stateWithSettings(settings: AppSettings) {
  return { ...structuredClone(browserState), settings: structuredClone(settings) };
}

// Open a category, returning to the menu when another category is open.
const openSection = (name: string) => {
  const back = screen.queryByRole("button", { name: "Back to settings" });
  if (back) fireEvent.click(back);
  const navigation = screen.queryByRole("navigation", { name: "Settings categories" }) ?? screen.getByRole("navigation", { name: "On this page" });
  fireEvent.click(within(navigation).getByRole("button", { name: new RegExp('^' + name + '(,|$)') }));
};
// The pointer settings live at the end of the shared Scanning settings.
const openMouse = async () => {
  fireEvent.click(screen.getByRole("button", { name: "Scanning" }));
  await screen.findByRole("slider", { name: "Pointer speed" });
};
const setPointerSpeed = (percent: number) => fireEvent.change(screen.getByRole("slider", { name: "Pointer speed" }), { target: { value: speedLevel(percent).toFixed(1) } });

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

  it("keeps the Mouse pointer settings on Scanning and Input on Settings", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    const navigation = screen.getByRole("navigation", { name: "Pages" });
    expect(within(navigation).getAllByRole("button").map((button) => button.textContent))
      .toEqual(["Home", "Switches", "Scanning", "Panels", "Mobile", "Settings", "Help"]);
    fireEvent.click(within(navigation).getByRole("button", { name: "Scanning" }));
    fireEvent.click(await screen.findByRole("button", { name: "Customise mouse scanning" }));
    expect(screen.getByText(/Actions opens editing, window, browser, media, display and scanning commands/)).toBeInTheDocument();
    expect(screen.getByText(/Modified clicks use the current pointer\./)).toBeInTheDocument();
    expect(screen.getByText(/press and release a switch to stop either action/)).toBeInTheDocument();
    expect(screen.getByText(/the stop switch keeps the action going until release/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Back to scanning settings" }));
    expect(screen.getByRole("heading", { name: "Mouse pointer", level: 2 })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: /Pointer speed/ })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Stop repeating" })).toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: "Click when I stop" })).not.toBeInTheDocument();
    fireEvent.click(within(navigation).getByRole("button", { name: "Settings" }));
    openSection("Input");
    expect(screen.getByRole("checkbox", { name: "Click when I stop" })).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: /Pointer speed/ })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "More options" }));
    expect(screen.getByRole("checkbox", { name: "Repeat held keys" })).toBeInTheDocument();
  });

  it("shows update progress and exposes cancellation in Settings", async () => {
    browserState.updater = { status: "downloading", version: "1.0.0-beta.2", downloadedBytes: 50, totalBytes: 200, error: null, retryAction: null };
    const cancel = vi.spyOn(api, "cancelUpdateDownload").mockResolvedValue(structuredClone(browserState));
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    openSection("Updates");

    expect(screen.getByText("Downloading Switchify PC 1.0.0-beta.2…")).toBeInTheDocument();
    expect(screen.getByRole("progressbar", { name: "Update download progress" })).toHaveAttribute("value", "50");
    expect(document.querySelector(".update-controls > span")).toHaveTextContent("25%");
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(cancel).toHaveBeenCalledOnce();
  });

  // Fake state and progress events; the download itself never runs.
  const fakeUpdateEvents = () => {
    let stateHandler: ((state: typeof browserState) => void) | undefined;
    const progressHandlers = new Set<(progress: { downloadedBytes: number; totalBytes: number | null }) => void>();
    vi.spyOn(api, "onState").mockImplementation(async (handler) => { stateHandler = handler; return () => undefined; });
    const onProgress = vi.spyOn(api, "onUpdateProgress").mockImplementation(async (handler) => {
      progressHandlers.add(handler);
      return () => { progressHandlers.delete(handler); };
    });
    return {
      progressHandlers,
      onProgress,
      sendProgress: (downloadedBytes: number, totalBytes: number | null) =>
        act(() => progressHandlers.forEach((handler) => handler({ downloadedBytes, totalBytes }))),
      sendUpdater: (updater: typeof browserState.updater) =>
        act(() => stateHandler?.({ ...structuredClone(browserState), updater })),
    };
  };
  const downloadingUpdater = { status: "downloading" as const, version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error: null, retryAction: null };

  it("shows the latest progress when Updates opens mid-download", async () => {
    const { progressHandlers, sendProgress } = fakeUpdateEvents();
    browserState.updater = downloadingUpdater;
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    await waitFor(() => expect(progressHandlers.size).toBe(1));

    sendProgress(150, 200);
    expect(screen.getByRole("status", { name: "Application update" })).toHaveTextContent("Downloading Switchify PC 1.0.0-beta.2 — 75%");
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    openSection("General");
    openSection("Updates");
    expect(screen.getByRole("progressbar", { name: "Update download progress" })).toHaveAttribute("value", "150");
    expect(document.querySelector(".update-controls > span")).toHaveTextContent("75%");
    expect(progressHandlers.size).toBe(1);
  });

  it.each([
    ["cancelled", { status: "cancelled" as const, version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error: null, retryAction: "download" as const }],
    ["failed", { status: "failed" as const, version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error: "Update download failed: offline", retryAction: "download" as const }],
  ])("starts a retried download at zero after it was %s", async (_name, ended) => {
    const { progressHandlers, sendProgress, sendUpdater } = fakeUpdateEvents();
    browserState.updater = downloadingUpdater;
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    openSection("Updates");
    await waitFor(() => expect(progressHandlers.size).toBe(1));
    sendProgress(150, 200);
    expect(screen.getByRole("progressbar", { name: "Update download progress" })).toHaveAttribute("value", "150");

    sendUpdater(ended);
    expect(screen.queryByRole("progressbar", { name: "Update download progress" })).toBeNull();
    sendUpdater(downloadingUpdater);
    expect(screen.getByRole("progressbar", { name: "Update download progress" })).toHaveAttribute("value", "0");
    expect(document.querySelector(".update-controls > span")).toHaveTextContent("0 bytes");
    sendProgress(20, 200);
    expect(document.querySelector(".update-controls > span")).toHaveTextContent("10%");
  });

  it("follows the progress event during a download and keeps the final size from state", async () => {
    const { progressHandlers, onProgress, sendProgress, sendUpdater } = fakeUpdateEvents();
    browserState.updater = downloadingUpdater;
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    openSection("Updates");
    await waitFor(() => expect(progressHandlers.size).toBe(1));

    sendProgress(1_000, null);
    expect(document.querySelector(".update-controls > span")).toHaveTextContent(`${(1_000).toLocaleString()} bytes`);
    sendProgress(150, 200);
    expect(screen.getByRole("progressbar", { name: "Update download progress" })).toHaveAttribute("value", "150");
    expect(screen.getByRole("progressbar", { name: "Update download progress" })).toHaveAttribute("max", "200");
    expect(document.querySelector(".update-controls > span")).toHaveTextContent("75%");
    expect(screen.getByRole("status", { name: "Application update" })).toHaveTextContent("Downloading Switchify PC 1.0.0-beta.2 — 75%");

    sendUpdater({ status: "readyToInstall", version: "1.0.0-beta.2", downloadedBytes: 200, totalBytes: 200, error: null, retryAction: null });
    expect(screen.queryByRole("progressbar", { name: "Update download progress" })).toBeNull();
    expect(screen.getByText("Switchify PC 1.0.0-beta.2 is ready to install.", { selector: ".update-controls p" })).toBeInTheDocument();
    expect(onProgress).toHaveBeenCalledOnce();

    // A new download starts from its own state, not the previous attempt's samples.
    sendUpdater(downloadingUpdater);
    expect(screen.getByRole("progressbar", { name: "Update download progress" })).toHaveAttribute("value", "0");
    sendProgress(20, 200);
    expect(document.querySelector(".update-controls > span")).toHaveTextContent("10%");
  });

  it("offers the correct retry action after a failure", async () => {
    browserState.updater = { status: "failed", version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error: "Download failed", retryAction: "download" };
    const download = vi.spyOn(api, "downloadUpdate").mockResolvedValue(structuredClone(browserState));
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    openSection("Updates");
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
    openSection("Updates");
    fireEvent.click(screen.getByRole("button", { name: "Install and restart" }));
    expect(install).toHaveBeenCalledOnce();
  });

  it("retries a cancelled download from the beginning", async () => {
    browserState.updater = { status: "cancelled", version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error: null, retryAction: "download" };
    const download = vi.spyOn(api, "downloadUpdate").mockResolvedValue(structuredClone(browserState));
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    openSection("Updates");
    expect(within(screen.getByRole("region", { name: "Updates" })).getByRole("status")).toHaveTextContent("Download cancelled. You can retry when ready.");
    fireEvent.click(screen.getByRole("button", { name: "Retry download" }));
    expect(download).toHaveBeenCalledOnce();
  });

  it("reports manual update-check failures in Settings", async () => {
    const checkForUpdates = vi.spyOn(api, "checkForUpdates").mockRejectedValue(new Error("Update service unavailable"));
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    openSection("Updates");
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
    openSection("Updates");

    expect(screen.getByText(description)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: action })).toBeInTheDocument();
  });

  it("exposes key repeat settings and disables them with the toggle", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("Input");
    fireEvent.click(screen.getByRole("button", { name: "More options" }));

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

    // Scanned keyboard repeat is off by default and shares the same timing.
    const scanned = screen.getByRole("checkbox", { name: "Repeat keys on the scanned keyboard" });
    expect(scanned).not.toBeChecked();
    fireEvent.click(scanned);
    expect(scanned).toBeChecked();
    expect(screen.getByRole("group", { name: "Delay before repeating" })).not.toBeDisabled();
    expect(screen.getByRole("group", { name: "Key interval" })).not.toBeDisabled();
  });

  it("opens the Mouse pointer settings on Scanning and keeps Input settings separate", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.getByRole("heading", { name: "Settings" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Save settings" })).not.toBeInTheDocument();
    openSection("General");
    expect(screen.getByRole("checkbox", { name: "Start with system" })).toBeInTheDocument();

    await openMouse();
    expect(screen.getByRole("heading", { name: "Mouse pointer", level: 2 })).toBeInTheDocument();
    expect(screen.getByText(/Pointer speed and repeat for Mouse scanning and Remote/)).toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: "Click when I stop" })).not.toBeInTheDocument();
    expect(screen.getByRole("checkbox", { name: "Repeat mouse movement and scrolling" })).toBeChecked();
    fireEvent.click(screen.getByRole("button", { name: "Repeat timing" }));
    expect(screen.getByRole("slider", { name: "Pointer speed" })).toHaveValue("5.0");
    expect(screen.getByRole("checkbox", { name: "Repeat mouse movement and scrolling" })).toBeChecked();
    expect(screen.getByRole("group", { name: "Movement acceleration" })).not.toBeDisabled();
    expect(screen.getAllByRole("button", { name: "Medium" })[0]).toHaveAttribute("aria-pressed", "true");
    setPointerSpeed(50);
    expect(screen.getByRole("slider", { name: "Pointer speed" })).toHaveValue("4.1");
    fireEvent.click(screen.getByRole("button", { name: "Fine tune speed" }));
    fireEvent.click(screen.getByRole("button", { name: "Faster" }));
    expect(screen.getByRole("slider", { name: "Pointer speed" })).toHaveAttribute("aria-valuetext", `Level ${speedLevel(55).toFixed(3)} of 10`);
    fireEvent.click(screen.getByRole("checkbox", { name: "Repeat mouse movement and scrolling" }));
    expect(screen.getByRole("group", { name: "Movement acceleration" })).toBeDisabled();
  });

  it("exposes the dwell controls and their explanatory note", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("Input");

    const dwell = screen.getByRole("checkbox", { name: "Click when I stop" });
    expect(dwell).not.toBeChecked();
    expect(screen.getByRole("group", { name: "Wait before clicking" })).toBeDisabled();
    fireEvent.click(dwell);
    const dwellDelay = screen.getByRole("group", { name: "Wait before clicking" });
    expect(dwellDelay).not.toBeDisabled();
    expect(within(dwellDelay).getAllByRole("button")).toHaveLength(10);
    for (const label of ["0.5s", "1s", "1.5s", "2s", "3s", "4s", "5s", "6s", "7s", "8s"]) {
      expect(within(dwellDelay).getByRole("button", { name: label })).toBeInTheDocument();
    }
    expect(within(dwellDelay).getByRole("button", { name: "1s" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(within(dwellDelay).getByRole("button", { name: "1.5s" }));
    expect(within(dwellDelay).getByRole("button", { name: "1.5s" })).toHaveAttribute("aria-pressed", "true");
    // Dwell's whole explanation fits one note, so it gets no disclosure, and
    // the group is described by it.
    const dwellNote = screen.getByText(/After mobile pointer movement stops/);
    expect(screen.queryByRole("button", { name: /about dwell/ })).not.toBeInTheDocument();
    expect(dwellDelay).toHaveAttribute("aria-describedby", dwellNote.id);
  });

  it("exposes the cursor overlay controls on the Mobile page", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));
    openSection("Cursor overlay");
    expect(screen.getByRole("heading", { name: "Cursor overlay", level: 2 })).toBeInTheDocument();

    expect(screen.getByRole("checkbox", { name: "Show cursor overlay" })).toBeChecked();
    expect(screen.getByRole("button", { name: "While controlling" })).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText("Choose when the overlay stays on screen.")).toBeInTheDocument();
    expect(screen.queryByText(/On input hides shortly/)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "More about overlay visibility" }));
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
    browserState.telemetry = { consent: "disabled", available: true };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("Privacy");

    fireEvent.click(screen.getByRole("checkbox", { name: "Share anonymous diagnostic data" }));

    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ shareDiagnostics: true })));
    expect(screen.getByRole("checkbox", { name: "Share anonymous diagnostic data" })).toBeChecked();
  });

  it("persists dwell enablement and delay through automatic settings saves", async () => {
    const saveSettings = vi.spyOn(api, "saveSettings").mockImplementation(async (settings) => stateWithSettings(settings));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("Input");

    fireEvent.click(screen.getByRole("checkbox", { name: "Click when I stop" }));
    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ dwellClickEnabled: true, dwellClickDelayMs: 1000 })));
    const dwellDelay = screen.getByRole("group", { name: "Wait before clicking" });
    fireEvent.click(within(dwellDelay).getByRole("button", { name: "8s" }));
    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ dwellClickEnabled: true, dwellClickDelayMs: 8000 })));
    expect(within(dwellDelay).getByRole("button", { name: "8s" })).toHaveAttribute("aria-pressed", "true");
  });

  it("shows the version, licence and project links on the About tab", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("About");

    const about = screen.getByRole("region", { name: "About" });
    expect(within(about).getByText(`Switchify PC ${browserState.version}`)).toBeInTheDocument();
    expect(within(about).getByRole("link", { name: "GNU Affero General Public License v3" })).toHaveAttribute("href", "https://github.com/switchifyapp/switchify-pc/blob/main/LICENSE");
    expect(within(about).getByRole("link", { name: "Source code" })).toHaveAttribute("href", "https://github.com/switchifyapp/switchify-pc");
    expect(within(about).getByRole("link", { name: "Licence and credits" })).toHaveAttribute("href", "https://pc.theswitchify.app/licence/");
    expect(within(about).getByRole("link", { name: "Online guide" })).toHaveAttribute("href", "https://pc.theswitchify.app");
  });

  it("explains telemetry consent and links to the privacy policy", async () => {
    const first = render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("Privacy");
    expect(screen.getByText(/Nothing is sent unless you choose Share diagnostics/)).toBeInTheDocument();
    // One affordance per state: only the choice buttons until consent is recorded.
    expect(screen.queryByRole("checkbox", { name: "Share anonymous diagnostic data" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Don't share" }));
    expect(await screen.findByText("Opted out. No diagnostic reports are stored or sent.")).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Privacy policy" })).toHaveAttribute("href", "https://www.theswitchify.app/privacy");
    // Once recorded, the toggle replaces the buttons.
    expect(screen.getByRole("checkbox", { name: "Share anonymous diagnostic data" })).not.toBeChecked();
    expect(screen.queryByRole("button", { name: "Share diagnostics" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Don't share" })).not.toBeInTheDocument();

    first.unmount();
    browserState.telemetry = { consent: "undecided", available: false };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("Privacy");
    expect(await screen.findByText("Diagnostic reporting is unavailable in this build.")).toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: "Share anonymous diagnostic data" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Share diagnostics" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Don't share" })).toBeEnabled();
  });

  it("serializes rapid settings changes without applying a stale response", async () => {
    const saves: Array<{ settings: AppSettings; resolve: (state: typeof browserState) => void }> = [];
    vi.spyOn(api, "saveSettings").mockImplementation((settings) => new Promise((resolve) => {
      saves.push({ settings: structuredClone(settings), resolve });
    }));
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    await openMouse();

    setPointerSpeed(50);
    setPointerSpeed(75);

    expect(saves).toHaveLength(1);
    expect(screen.getByRole("slider", { name: "Pointer speed" })).toHaveValue("4.6");

    await act(async () => {
      saves[0].resolve(stateWithSettings(saves[0].settings));
    });
    await waitFor(() => expect(saves).toHaveLength(2));
    expect(saves[1].settings.pointerScalePercent).toBe(75);
    expect(screen.getByRole("slider", { name: "Pointer speed" })).toHaveValue("4.6");

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
    await screen.findByRole("heading", { name: "Switchify PC" });
    await openMouse();

    setPointerSpeed(50);
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
    browserState.telemetry = { consent: "disabled", available: true };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    openSection("Privacy");
    fireEvent.click(screen.getByRole("checkbox", { name: "Share anonymous diagnostic data" }));
    await openMouse();
    setPointerSpeed(50);
    act(() => stateHandler?.(stateWithSettings({ ...defaultBrowserSettings, pointerScalePercent: 150 })));

    expect(screen.getByRole("slider", { name: "Pointer speed" })).toHaveValue("5.8");
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    openSection("Privacy");
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

    openSection("General");
    const startup = screen.getByRole("checkbox", { name: "Start with system" });
    fireEvent.click(startup);
    expect(startup).toBeChecked();

    expect(await screen.findByRole("alert")).toHaveTextContent("Settings storage unavailable");
    expect(startup).not.toBeChecked();
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
    expect(screen.queryByRole("region", { name: "Updates" })).not.toBeInTheDocument();
  });


  it("opens one category at a time and returns focus to its menu button", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    const menu = screen.getByRole("navigation", { name: "Settings categories" });
    expect(within(menu).getAllByRole("button").map((button) => button.getAttribute("aria-label")))
      .toEqual(["General", "Input", "Privacy", "Updates", "Licence", "About"]);
    expect(screen.queryByRole("checkbox")).not.toBeInTheDocument();
    expect(screen.queryByRole("region", { name: "Updates" })).not.toBeInTheDocument();
    openSection("General");
    expect(screen.getByRole("heading", { name: "General" })).toHaveFocus();
    expect(screen.getByRole("checkbox", { name: "Start with system" })).toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: "Click when I stop" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Back to settings" }));
    expect(screen.getByRole("button", { name: "General" })).toHaveFocus();
    openSection("Input");
    expect(screen.getByRole("checkbox", { name: "Click when I stop" })).toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: "Start with system" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.getByRole("navigation", { name: "Settings categories" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Input" })).toHaveFocus();
  });

  it.each([
    ["startup", "Start with system", "startup", "General"],
    ["dwell", "Click when I stop", "dwell", "Input"],
    ["privacy", "Anonymous diagnostics", "privacy", "Privacy"],
    ["licence", "Licence", "licence", "Licence"],
  ])("opens the exact %s setting from Find and returns to its category", async (query, label, setting, category) => {
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    fireEvent.change(screen.getByRole("searchbox", { name: "Find a setting" }), { target: { value: query } });
    const results = await screen.findByRole("list", { name: "Matching settings" });
    fireEvent.click(within(results).getByRole("button", { name: new RegExp(`^${label}, Settings,`) }));
    await waitFor(() => expect(document.querySelector(`[data-setting="${setting}"]`)).toHaveAttribute("data-found", "true"));
    const target = document.querySelector(`[data-setting="${setting}"]`);
    expect(target?.contains(document.activeElement)).toBe(true);
    expect(screen.queryByRole("navigation", { name: "Settings categories" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Back to settings" }));
    expect(screen.getByRole("button", { name: category })).toHaveFocus();
  });

  it.each([
    ["General", "typing"], ["General", "button"],
    ["Input", "typing"], ["Input", "button"],
  ])("keeps Find focused when clearing search with %s open using %s", async (category, method) => {
    render(<StrictMode><App /></StrictMode>);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection(category);
    const find = screen.getByRole("searchbox", { name: "Find a setting" });
    find.focus();
    fireEvent.change(find, { target: { value: "dw" } });
    await screen.findByRole("list", { name: "Matching settings" });
    expect(screen.queryByRole("button", { name: "Back to settings" })).not.toBeInTheDocument();
    if (method === "typing") fireEvent.change(find, { target: { value: "" } });
    else fireEvent.click(screen.getByRole("button", { name: "Clear search" }));
    await screen.findByRole("heading", { name: category, level: 2 });
    expect(find).toHaveFocus();
    expect(screen.getByRole("button", { name: "Back to settings" })).toBeInTheDocument();
  });

  it("omits the cursor overlay section when the platform cannot show a cursor overlay", async () => {
    browserState.capabilities = { ...structuredClone(defaultCapabilities), cursorOverlay: false };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));

    const strip = screen.getByRole("navigation", { name: "On this page" });
    expect(within(strip).queryByRole("button", { name: /^Cursor overlay/ })).not.toBeInTheDocument();
    expect(screen.queryByRole("checkbox", { name: "Show cursor overlay" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(screen.queryByRole("checkbox", { name: "Show cursor overlay" })).not.toBeInTheDocument();
  });

  it("focuses Updates when the banner is used from another section", async () => {
    browserState.updater = { status: "available", version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error: null, retryAction: null };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("Input");

    fireEvent.click(within(screen.getByRole("status", { name: "Application update" })).getByRole("button", { name: "View update" }));

    await waitFor(() => expect(screen.getByRole("region", { name: "Updates" })).toHaveFocus());
  });

  it("keeps a pending Mouse edit across pages and saves it", async () => {
    const saveSettings = vi.spyOn(api, "saveSettings").mockImplementation(async (settings) => stateWithSettings(settings));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    await openMouse();
    setPointerSpeed(50);
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    openSection("Privacy");
    await openMouse();

    expect(screen.getByRole("slider", { name: "Pointer speed" })).toHaveValue("4.1");
    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ pointerScalePercent: 50 })));
  });


  it("shows an accessible Slow–Fast scale and keeps fine tuning collapsed", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    await openMouse();
    const slider = screen.getByRole("slider", { name: "Pointer speed" });
    expect(slider).toHaveValue("5.0");
    expect(slider).toHaveAttribute("aria-valuetext", "Level 5.000 of 10");
    expect(screen.getByRole("group", { name: /Pointer speed/ })).toHaveTextContent("Level 5.0 of 10");
    expect(screen.getByText("Slow")).toBeInTheDocument();
    expect(screen.getByText("Fast")).toBeInTheDocument();
    expect(screen.queryByText(/100%/)).not.toBeInTheDocument();
    const disclosure = screen.getByRole("button", { name: "Fine tune speed" });
    expect(disclosure).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(disclosure);
    expect(disclosure).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("button", { name: "Slower" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Faster" })).toBeEnabled();
  });

  it("preserves older saved speeds until the user changes them", async () => {
    const save = vi.spyOn(api, "saveSettings");
    browserState.settings = { ...structuredClone(defaultBrowserSettings), pointerScalePercent: 150 };
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    await openMouse();
    expect(screen.getByRole("slider", { name: "Pointer speed" })).toHaveValue("5.8");
    expect(save).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Fine tune speed" }));
    fireEvent.click(screen.getByRole("button", { name: "Faster" }));
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ pointerScalePercent: 155 })));
  });

  it("advances from the slow endpoint with the keyboard and retains a drag position on a rounding plateau", async () => {
    const save = vi.spyOn(api, "saveSettings");
    browserState.settings = { ...structuredClone(defaultBrowserSettings), pointerScalePercent: 5 };
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    await openMouse();
    const slider = screen.getByRole("slider", { name: "Pointer speed" });
    fireEvent.change(slider, { target: { value: "1.1" } });
    expect(slider).toHaveValue("1.1");
    expect(save).not.toHaveBeenCalled();
    fireEvent.keyDown(slider, { key: "ArrowRight" });
    await waitFor(() => expect(save).toHaveBeenCalledWith(expect.objectContaining({ pointerScalePercent: 10 })));
    expect(slider).toHaveAttribute("aria-valuetext", `Level ${speedLevel(10).toFixed(3)} of 10`);
  });

  it("announces every fine tuning step even when the rounded main level stays the same", async () => {
    browserState.settings = { ...structuredClone(defaultBrowserSettings), pointerScalePercent: 1300 };
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    await openMouse();
    fireEvent.click(screen.getByRole("button", { name: "Fine tune speed" }));
    const before = screen.getByText(`Fine-tuned level ${speedLevel(1300).toFixed(3)} of 10`).textContent;
    fireEvent.click(screen.getByRole("button", { name: "Faster" }));
    expect(screen.getByText(`Fine-tuned level ${speedLevel(1305).toFixed(3)} of 10`)).toBeInTheDocument();
    expect(screen.getByText(`Fine-tuned level ${speedLevel(1305).toFixed(3)} of 10`).textContent).not.toBe(before);
    expect(screen.getByRole("slider", { name: "Pointer speed" })).toHaveAttribute("aria-valuetext", `Level ${speedLevel(1305).toFixed(3)} of 10`);
  });

  it("maps both slider endpoints and stops fine tuning at the limits", async () => {
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    await openMouse();
    const slider = screen.getByRole("slider", { name: "Pointer speed" });
    fireEvent.change(slider, { target: { value: "1" } });
    expect(slider).toHaveValue("1.0");
    fireEvent.click(screen.getByRole("button", { name: "Fine tune speed" }));
    expect(screen.getByRole("button", { name: "Slower" })).toBeDisabled();
    fireEvent.change(slider, { target: { value: "10" } });
    expect(slider).toHaveValue("10.0");
    expect(screen.getByRole("button", { name: "Faster" })).toBeDisabled();
  });

  it("reflects a backend speed change without overwriting it", async () => {
    let stateHandler: ((state: typeof browserState) => void) | undefined;
    vi.spyOn(api, "onState").mockImplementation(async (handler) => {
      stateHandler = handler;
      return () => undefined;
    });
    render(<App />);
    await screen.findByRole("heading", { name: "Switchify PC" });
    await openMouse();
    act(() => stateHandler?.(stateWithSettings({ ...defaultBrowserSettings, pointerScalePercent: 175 })));
    expect(screen.getByRole("slider", { name: "Pointer speed" })).toHaveValue("6.1");
  });

  it("maps speed levels monotonically to valid saved values", () => {
    expect(speedPercent(1)).toBe(5);
    expect(speedPercent(5)).toBe(100);
    expect(speedPercent(10)).toBe(1350);
    expect(speedLevel(100)).toBe(5);
    let previous = 0;
    for (let tenth = 10; tenth <= 100; tenth++) {
      const percent = speedPercent(tenth / 10);
      expect(percent).toBeGreaterThanOrEqual(previous);
      expect(percent % 5).toBe(0);
      previous = percent;
    }
  });

  it("keeps the key repeat explanation behind a disclosure", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("Input");
    fireEvent.click(screen.getByRole("button", { name: "More options" }));

    expect(screen.getByText("Held navigation keys repeat, like on a keyboard.")).toBeInTheDocument();
    expect(screen.queryByText(/Applies to the arrow keys/)).not.toBeInTheDocument();

    const more = screen.getByRole("button", { name: "More about key repeat" });
    expect(more).toHaveAttribute("aria-expanded", "false");
    fireEvent.click(more);

    expect(screen.getByText(/Applies to the arrow keys, Tab, Backspace, Delete, Page Up, and Page Down/)).toBeInTheDocument();
    const less = screen.getByRole("button", { name: "Show less about key repeat" });
    expect(less).toHaveAttribute("aria-expanded", "true");
    fireEvent.click(less);
    expect(screen.queryByText(/Applies to the arrow keys/)).not.toBeInTheDocument();
  });

  it("keeps the privacy consent text visible rather than behind a disclosure", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("Privacy");

    // Consent legibility, not clutter: this must never move behind a disclosure.
    expect(screen.getByText(/Nothing is sent unless you choose Share diagnostics/)).toBeInTheDocument();
    expect(screen.getByRole("link", { name: "Privacy policy" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /More about/ })).not.toBeInTheDocument();
  });


  it("keeps the overlay explanation reachable while the overlay is off", async () => {
    browserState.settings = { ...structuredClone(defaultBrowserSettings), cursorOverlayEnabled: false };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Mobile" }));

    // The disclosure must sit outside the disabled fieldset. Asserted on the
    // disabled property rather than by clicking, because jsdom dispatches clicks
    // on disabled buttons and a browser does not.
    const more = screen.getByRole("button", { name: "More about overlay visibility" });
    expect(more).not.toBeDisabled();
    expect(screen.getByRole("group", { name: "Overlay visibility" })).toBeDisabled();

    fireEvent.click(more);
    expect(screen.getByText(/On input hides shortly/)).toBeInTheDocument();
  });

  it("gives each help disclosure a distinct accessible name", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("Input");
    fireEvent.click(screen.getByRole("button", { name: "More options" }));
    const pointerNames = screen.getAllByRole("button", { name: /More about/ }).map((button) => button.textContent);
    expect(pointerNames).toEqual(["More about key repeat"]);
    fireEvent.click(screen.getByRole("button", { name: "Mobile" }));
    const cursorNames = screen.getAllByRole("button", { name: /More about/ }).map((button) => button.textContent);
    expect(cursorNames).toEqual(["More about overlay visibility"]);
    expect(new Set([...pointerNames, ...cursorNames]).size).toBe(2);
  });


  it("links each help disclosure to the detail it reveals and the group to its summary", async () => {
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("Input");
    fireEvent.click(screen.getByRole("button", { name: "More options" }));

    // The group is described by the always-visible summary.
    const summary = screen.getByText("Held navigation keys repeat, like on a keyboard.");
    expect(screen.getByRole("group", { name: "Key interval" })).toHaveAttribute("aria-describedby", summary.id);

    // Collapsed, the detail is unmounted, so the button must not point at it.
    const more = screen.getByRole("button", { name: "More about key repeat" });
    expect(more).toHaveAttribute("aria-expanded", "false");
    expect(more).not.toHaveAttribute("aria-controls");
    fireEvent.click(more);

    // Open, it points at the detail, which adds to the summary rather than
    // repeating it, so the summary stays put and nothing is read twice.
    const less = screen.getByRole("button", { name: "Show less about key repeat" });
    const detail = document.getElementById(less.getAttribute("aria-controls")!)!;
    expect(detail).toHaveTextContent("Applies to the arrow keys, Tab, Backspace, Delete, Page Up, and Page Down.");
    expect(detail).not.toHaveTextContent("like on a keyboard");
    expect(summary).toBeInTheDocument();
  });


  const failedUpdater = (error: string) => ({ status: "failed" as const, version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error, retryAction: "download" as const });
  const updatesNotice = () => document.getElementById("settings-updates-notice")!;
  it("opens straight to Updates without a notice when the failure is what brought the user there", async () => {
    browserState.updater = failedUpdater("Download failed");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Help" }));
    fireEvent.click(screen.getByRole("button", { name: "View updates" }));

    await waitFor(() => expect(screen.getByRole("region", { name: "Updates" })).toHaveFocus());
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(screen.getByRole("alert")).toHaveTextContent("Download failed.");
    expect(updatesNotice()).toBeEmptyDOMElement();
  });

  it("announces a standing failure in the menu and shows its details only in Updates", async () => {
    browserState.updater = failedUpdater("Download failed");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(updatesNotice()).toHaveTextContent("Download failed. Open Updates in Settings to retry.");
    openSection("General");
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    openSection("Updates");
    expect(screen.getAllByRole("alert")).toHaveLength(1);
    expect(screen.getByRole("alert")).toHaveTextContent("Download failed.");
    expect(updatesNotice()).toBeEmptyDOMElement();
    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    expect(updatesNotice()).toBeEmptyDOMElement();
  });

  it("ends the backend failure text as a sentence everywhere it is shown", async () => {
    browserState.updater = failedUpdater("Update download could not start: check for an update first");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("Updates");
    expect(screen.getByRole("alert")).toHaveTextContent("Update download could not start: check for an update first.");
    expect(updatesNotice()).toBeEmptyDOMElement();
  });

  it("reports a cancelled download politely without repeating the retry hint", async () => {
    browserState.updater = { status: "cancelled", version: "1.0.0-beta.2", downloadedBytes: 0, totalBytes: null, error: null, retryAction: "download" };
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    openSection("Updates");

    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(within(screen.getByRole("region", { name: "Updates" })).getByRole("status")).toHaveTextContent("Download cancelled. You can retry when ready.");
    expect(updatesNotice()).toBeEmptyDOMElement();
  });

});
