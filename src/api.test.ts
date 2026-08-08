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
