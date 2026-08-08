import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import { api, browserState } from "./api";
import type { AppSettings } from "./types";

const defaultBrowserSettings = structuredClone(browserState.settings);

function stateWithSettings(settings: AppSettings) {
  return { ...structuredClone(browserState), settings: structuredClone(settings) };
}

describe("Switchify PC shell", () => {
  beforeEach(() => {
    browserState.settings = structuredClone(defaultBrowserSettings);
    browserState.pendingPairings = [];
    browserState.connectedDeviceName = null;
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
    expect(dialog).toHaveFocus();
    expect(screen.getByText("Connected to Connected tablet")).toBeInTheDocument();
    const devices = within(dialog).getAllByRole("heading", { level: 3 });
    expect(devices.map((heading) => heading.textContent)).toEqual(["Galaxy", "Pixel"]);

    fireEvent.click(screen.getByRole("button", { name: "Accept pairing request from Galaxy, code 222222" }));

    await waitFor(() => expect(screen.queryByRole("heading", { name: "Galaxy" })).not.toBeInTheDocument());
    expect(approve).toHaveBeenCalledWith("pair-2");
    expect(screen.getByRole("heading", { name: "Pixel" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Reject pairing request from Pixel, code 111111" })).toHaveFocus();
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

    fireEvent.click(screen.getByRole("checkbox", { name: "Share diagnostic data" }));

    await waitFor(() => expect(saveSettings).toHaveBeenCalledWith(expect.objectContaining({ shareDiagnostics: true })));
    expect(screen.getByRole("checkbox", { name: "Share diagnostic data" })).toBeChecked();
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

    fireEvent.click(screen.getByRole("checkbox", { name: "Share diagnostic data" }));
    fireEvent.click(screen.getByRole("button", { name: "50% pointer speed" }));
    act(() => stateHandler?.(stateWithSettings({ ...defaultBrowserSettings, pointerScalePercent: 150 })));

    expect(screen.getByRole("checkbox", { name: "Share diagnostic data" })).toBeChecked();
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
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Switch control" }));
    const newProfile = await screen.findByRole("button", { name: "New profile" });
    expect(newProfile).toHaveClass("primary");
    fireEvent.click(newProfile);
    fireEvent.change(screen.getByRole("combobox", { name: "Switch 1 action" }), { target: { value: "shortcut" } });
    const recorder = screen.getByRole("textbox", { name: "Switch 1 key" });
    fireEvent.keyDown(recorder, { key: "K", ctrlKey: true });
    expect(recorder).toHaveValue("Ctrl+K");
    fireEvent.click(screen.getByRole("button", { name: "Save profile" }));
    expect(await screen.findByText("New profile")).toBeInTheDocument();
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
