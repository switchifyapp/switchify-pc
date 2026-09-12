import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { beforeEach, afterEach, it, expect, vi } from "vitest";
import { useSwitches, type SwitchState } from "./scanning/useSwitches";
import { useScanning, defaultPointScanConfig } from "./scanning/useScanning";
import { SwitchesSection } from "./settings/SwitchesSection";
import { ScanningSection } from "./settings/ScanningSection";
const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
const initial: SwitchState = {
  settings: {
    schemaVersion: 1,
    holdIntervalMs: 1000,
    bindings: [
      {
        id: "one",
        name: "Head switch",
        key: "Space",
        pressAction: "select",
        holdActions: ["next", "stop"],
      },
    ],
  },
  capture: { active: false, key: null, error: null },
  supported: true,
  error: null,
  escapeHoldMs: 4000,
  unavailableKeys: [],
};
let current: SwitchState;
const scan = {
  config: defaultPointScanConfig,
  enabled: false,
  phase: "idle",
  paused: false,
  message: "Scanning is off.",
  supported: true,
};
function Shell({ visible = true }: { visible?: boolean }) {
  const switches = useSwitches();
  const scanning = useScanning(switches.flush);
  switches.setLocked(!!scanning.state?.enabled || scanning.toggling);
  return (
    <>
      {visible && (
        <SwitchesSection
          controller={switches}
          locked={!!scanning.state?.enabled || scanning.toggling}
        />
      )}
      <ScanningSection controller={scanning} />
    </>
  );
}
function event(next: SwitchState) {
  act(() =>
    mocks.listen.mock.calls.find(([name]) => name === "switches-changed")![1]({
      payload: next,
    }),
  );
}
beforeEach(() => {
  current = structuredClone(initial);
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    configurable: true,
    value: {},
  });
  mocks.listen.mockReset().mockResolvedValue(vi.fn());
  mocks.invoke.mockReset().mockImplementation(async (command, args) => {
    switch (command) {
      case "get_switches":
        return current;
      case "save_switches":
        current = { ...current, settings: args.settings };
        return current;
      case "begin_switch_capture":
        current = {
          ...current,
          capture: { active: true, key: null, error: null },
        };
        return current;
      case "cancel_switch_capture":
        current = {
          ...current,
          capture: { active: false, key: null, error: null },
        };
        return current;
      case "get_point_scan":
        return scan;
      case "configure_point_scan":
        return { ...scan, ...args };
    }
  });
});
afterEach(() => Reflect.deleteProperty(window, "__TAURI_INTERNALS__"));
it("commits a new switch only after name and physical capture are complete", async () => {
  render(<Shell />);
  await screen.findByText("Key: Space");
  fireEvent.click(screen.getByRole("button", { name: "Add switch" }));
  fireEvent.change(screen.getByLabelText("New switch name"), {
    target: { value: "Foot switch" },
  });
  expect(mocks.invoke.mock.calls.some(([c]) => c === "save_switches")).toBe(
    false,
  );
  fireEvent.click(screen.getByRole("button", { name: "Learn switch key" }));
  await screen.findByRole("button", { name: "Cancel capture" });
  event({ ...current, capture: { active: false, key: "Enter", error: null } });
  await screen.findByText("Key: Enter");
  fireEvent.click(screen.getByRole("button", { name: "Save new switch" }));
  await waitFor(() => expect(current.settings.bindings).toHaveLength(2));
  expect(current.settings.bindings[1]).toMatchObject({
    name: "Foot switch",
    key: "Enter",
    pressAction: "select",
  });
});
it("rejects a learned duplicate without changing the existing switch", async () => {
  render(<Shell />);
  await screen.findByText("Key: Space");
  fireEvent.click(screen.getByRole("button", { name: "Add switch" }));
  fireEvent.click(screen.getByRole("button", { name: "Learn switch key" }));
  await screen.findByRole("button", { name: "Cancel capture" });
  event({ ...current, capture: { active: false, key: "Space", error: null } });
  await screen.findByText("That key already belongs to another switch.");
  expect(current.settings.bindings).toHaveLength(1);
});
it("reorders hold actions and waits for pending switch saves before enabling", async () => {
  render(<Shell />);
  await screen.findByText("Key: Space");
  let finish!: (v: SwitchState) => void;
  mocks.invoke.mockImplementationOnce(
    () =>
      new Promise<SwitchState>((r) => {
        finish = r;
      }),
  );
  fireEvent.click(
    screen.getByRole("button", { name: "Move hold action 2 up" }),
  );
  await waitFor(() => expect(finish).toBeTypeOf("function"));
  fireEvent.click(screen.getByRole("button", { name: "Enable point scan" }));
  expect(
    mocks.invoke.mock.calls.some(
      ([c, a]) => c === "configure_point_scan" && a.enabled,
    ),
  ).toBe(false);
  await act(async () =>
    finish({
      ...current,
      settings: {
        ...current.settings,
        bindings: [
          { ...current.settings.bindings[0], holdActions: ["stop", "next"] },
        ],
      },
    }),
  );
  await screen.findByRole("button", { name: "Disable point scan" });
  expect(screen.getByLabelText("Normal action for Head switch")).toBeDisabled();
});
it("preserves failed switch edits and prevents enable until retry succeeds", async () => {
  render(<Shell />);
  await screen.findByText("Key: Space");
  mocks.invoke.mockRejectedValueOnce("Disk full");
  fireEvent.change(screen.getByLabelText("Name for Space"), {
    target: { value: "New name" },
  });
  await screen.findByText("Disk full");
  fireEvent.click(screen.getByRole("button", { name: "Enable point scan" }));
  await screen.findByText(/Save switch assignments before enabling/);
  expect(
    mocks.invoke.mock.calls.some(
      ([c, a]) => c === "configure_point_scan" && a.enabled,
    ),
  ).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Retry save" }));
  await waitFor(() =>
    expect(current.settings.bindings[0].name).toBe("New name"),
  );
});
it("does not reuse an old learned key when starting another capture", async () => {
  current = { ...current, capture: { active: false, key: "F2", error: null } };
  render(<Shell />);
  await screen.findByText("Key: Space");
  fireEvent.click(
    screen.getByRole("button", { name: "Learn another key for Head switch" }),
  );
  await screen.findByRole("button", { name: "Cancel capture" });
  expect(current.settings.bindings[0].key).toBe("Space");
  event({ ...current, capture: { active: false, key: "F3", error: null } });
  await waitFor(() => expect(current.settings.bindings[0].key).toBe("F3"));
});
it("cancels learning when the panel unmounts and retains pending edits", async () => {
  const view = render(<Shell />);
  await screen.findByText("Key: Space");
  fireEvent.click(
    screen.getByRole("button", { name: "Learn another key for Head switch" }),
  );
  await screen.findByRole("button", { name: "Cancel capture" });
  view.rerender(<Shell visible={false} />);
  await waitFor(() =>
    expect(mocks.invoke).toHaveBeenCalledWith("cancel_switch_capture"),
  );
  expect(current.capture.active).toBe(false);
});
