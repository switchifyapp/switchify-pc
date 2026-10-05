import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useRemoteSwitches, type RemoteConfig } from "./useRemoteSwitches";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

const slots = (first: string | null) => Array.from({ length: 8 }, (_, index) => ({
  pressAction: index === 0 ? first : null,
  holdActions: [],
}));
const config = (first: string | null, revision = 1): RemoteConfig => ({ schemaVersion: 1, revision, slots: slots(first) as RemoteConfig["slots"] });

let stored: RemoteConfig;
const syncEvent = () => mocks.listen.mock.calls.find(([name]) => name === "remote-switches-changed")![1]();

beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  stored = config("select");
  mocks.listen.mockReset().mockResolvedValue(vi.fn());
  mocks.invoke.mockReset().mockImplementation(async (command: string, args?: { config: RemoteConfig }) => {
    if (command === "get_remote_switches") return structuredClone(stored);
    if (command === "save_remote_switches") {
      stored = { ...args!.config, revision: stored.revision + 1 };
      return structuredClone(stored);
    }
    throw new Error(command);
  });
});
afterEach(() => Reflect.deleteProperty(window, "__TAURI_INTERNALS__"));

it("reloads slots that settings sync replaced", async () => {
  const { result } = renderHook(() => useRemoteSwitches());
  await waitFor(() => expect(result.current.config?.slots[0].pressAction).toBe("select"));
  stored = config("next", 5);
  await act(async () => { syncEvent(); });
  await waitFor(() => expect(result.current.config?.slots[0].pressAction).toBe("next"));
});

it("keeps edits that are still saving instead of reloading over them", async () => {
  let finishSave: () => void = () => undefined;
  mocks.invoke.mockImplementation(async (command: string, args?: { config: RemoteConfig }) => {
    if (command === "get_remote_switches") return structuredClone(stored);
    await new Promise<void>((resolve) => { finishSave = resolve; });
    stored = { ...args!.config, revision: stored.revision + 1 };
    return structuredClone(stored);
  });
  const { result } = renderHook(() => useRemoteSwitches());
  await waitFor(() => expect(result.current.config).not.toBeNull());
  act(() => result.current.update(slots("back") as RemoteConfig["slots"]));
  await act(async () => { syncEvent(); });
  expect(result.current.config?.slots[0].pressAction).toBe("back");
  expect(mocks.invoke.mock.calls.filter(([name]) => name === "get_remote_switches")).toHaveLength(1);
  await act(async () => { finishSave(); });
});
