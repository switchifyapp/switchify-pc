import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { defaultPointScanConfig, useScanning, type PointScanState } from "./useScanning";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

const state = (config = defaultPointScanConfig): PointScanState => ({ config, supported: true, enabled: false, paused: false, phase: "idle", message: "" });

beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  mocks.listen.mockReset().mockResolvedValue(vi.fn());
  mocks.invoke.mockReset().mockImplementation(async (command: string, args?: { config: typeof defaultPointScanConfig }) => {
    if (command === "get_point_scan") return state();
    if (command === "configure_point_scan") return state(args!.config);
    throw new Error(command);
  });
});
afterEach(() => { delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; });

it("saves two changes made together, such as a setup pace, in one configuration", async () => {
  const { result } = renderHook(() => useScanning());
  await waitFor(() => expect(result.current.state).not.toBeNull());
  act(() => {
    result.current.update("blockIntervalMs", 2500);
    result.current.update("speed", 0);
  });
  await waitFor(() => expect(result.current.pending).toBe(0));
  const saves = mocks.invoke.mock.calls.filter(([command]) => command === "configure_point_scan");
  expect(saves).toHaveLength(1);
  expect(saves[0][1].config).toMatchObject({ blockIntervalMs: 2500, speed: 0 });
  expect(result.current.unsaved).toBe(false);
});
