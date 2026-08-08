import { afterEach, describe, expect, it, vi } from "vitest";
import { listen } from "@tauri-apps/api/event";
import { api } from "./api";

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async () => () => undefined),
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
});
