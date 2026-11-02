import { afterEach, describe, expect, it, vi } from "vitest";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { api } from "./api";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => undefined),
}));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => null),
}));

describe("runtime state events", () => {
  afterEach(() => {
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    vi.clearAllMocks();
  });

  it("subscribes to the promoted application event", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });

    await api.onState(() => undefined);

    expect(listen).toHaveBeenCalledWith("app-state-changed", expect.any(Function));
  });

  it("subscribes to native profile exit requests", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });

    await api.onProfileExitRequested(() => undefined);

    expect(listen).toHaveBeenCalledWith("profile-exit-requested", expect.any(Function));
  });

  it("subscribes to tray navigation requests", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });

    await api.onNavigateRequested(() => undefined);

    expect(listen).toHaveBeenCalledWith("navigate-requested", expect.any(Function));
    expect(invoke).toHaveBeenCalledWith("take_navigation_request");
  });

  it("invokes each updater lifecycle command", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });

    await api.checkForUpdates();
    await api.downloadUpdate();
    await api.cancelUpdateDownload();
    await api.installUpdate();

    expect(invoke).toHaveBeenCalledWith("check_for_updates", undefined);
    expect(invoke).toHaveBeenCalledWith("download_update", undefined);
    expect(invoke).toHaveBeenCalledWith("cancel_update_download", undefined);
    expect(invoke).toHaveBeenCalledWith("install_update", undefined);
  });

  it("delivers tray navigation queued before the listener was ready", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", {
      configurable: true,
      value: {},
    });
    vi.mocked(invoke).mockResolvedValueOnce("settings");
    const handler = vi.fn();

    await api.onNavigateRequested(handler);

    expect(handler).toHaveBeenCalledWith("settings");
  });
});

describe("panel action library", () => {
  afterEach(() => {
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    vi.clearAllMocks();
  });

  it("asks the backend for the catalogue with the search words", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.mocked(invoke).mockResolvedValueOnce([]);
    await api.panelActions("alt tab");
    expect(invoke).toHaveBeenCalledWith("get_panel_actions", { query: "alt tab" });
    await api.panelActions();
    expect(invoke).toHaveBeenLastCalledWith("get_panel_actions", { query: null });
  });

  it("checks a built action through the backend", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.mocked(invoke).mockResolvedValueOnce("Ctrl+S");
    await expect(api.checkPanelAction({ type: "key", keys: ["Ctrl", "S"] })).resolves.toBe("Ctrl+S");
    expect(invoke).toHaveBeenCalledWith("check_panel_action", { action: { type: "key", keys: ["Ctrl", "S"] } });
  });

  it("has an empty library in a plain browser", async () => {
    await expect(api.panelActions("copy")).resolves.toEqual([]);
  });
});

describe("custom panels", () => {
  afterEach(() => {
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    vi.clearAllMocks();
  });

  const panel = {
    id: "greetings",
    version: 1,
    name: "Greetings",
    description: "",
    rows: [[{ label: "Hello", action: { type: "text" as const, text: "Hello there" }, weight: 1 }]],
  };

  it("saves, lists and deletes through the backend", async () => {
    Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
    vi.mocked(invoke).mockResolvedValue([panel]);
    await expect(api.savePanel(panel)).resolves.toEqual([panel]);
    expect(invoke).toHaveBeenCalledWith("save_custom_panel", { panel });
    await api.listPanels();
    expect(invoke).toHaveBeenLastCalledWith("list_custom_panels", undefined);
    await api.deletePanel("greetings");
    expect(invoke).toHaveBeenLastCalledWith("delete_custom_panel", { panelId: "greetings" });
    await api.onPanelsChanged(() => undefined);
    expect(listen).toHaveBeenCalledWith("custom-panels-changed", expect.any(Function));
  });

  it("keeps panels in memory in a plain browser and bumps the version", async () => {
    await expect(api.listPanels()).resolves.toEqual([]);
    const saved = await api.savePanel(panel);
    expect(saved[0].version).toBe(1);
    const again = await api.savePanel({ ...panel, name: "Hi" });
    expect(again[0]).toMatchObject({ name: "Hi", version: 2 });
    await expect(api.deletePanel("greetings")).resolves.toEqual([]);
  });
});
