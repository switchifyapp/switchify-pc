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
const open = (name: string) =>
  fireEvent.click(screen.getByRole("button", { name: `Edit ${name}` }));
it("shows each switch as a summary row and expands one to edit", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  expect(screen.getByText("Select · Hold: Next, Stop scanning")).toBeTruthy();
  expect(screen.queryByLabelText("Name for Space")).toBeNull();
  open("Head switch");
  expect(screen.getByLabelText("Name for Space")).toBeTruthy();
  expect(
    screen.getByRole("button", { name: "Close Head switch" }).getAttribute("aria-expanded"),
  ).toBe("true");
  fireEvent.click(screen.getByRole("button", { name: "Done" }));
  expect(screen.queryByLabelText("Name for Space")).toBeNull();
});
it("starts learning when a switch is added and commits only after name and key", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  fireEvent.click(screen.getByRole("button", { name: "Add switch" }));
  await screen.findByRole("button", { name: "Cancel capture" });
  expect(mocks.invoke).toHaveBeenCalledWith("begin_switch_capture");
  expect(screen.getByRole("button", { name: "Save switch" })).toBeDisabled();
  event({ ...current, capture: { active: false, key: "Enter", error: null } });
  await screen.findByText("Enter");
  expect(screen.getByRole("button", { name: "Save switch" })).toBeDisabled();
  fireEvent.change(screen.getByLabelText("New switch name"), {
    target: { value: "Foot switch" },
  });
  expect(mocks.invoke.mock.calls.some(([c]) => c === "save_switches")).toBe(
    false,
  );
  fireEvent.click(screen.getByRole("button", { name: "Save switch" }));
  await waitFor(() => expect(current.settings.bindings).toHaveLength(2));
  expect(current.settings.bindings[1]).toMatchObject({
    name: "Foot switch",
    key: "Enter",
    pressAction: "select",
  });
  expect(screen.queryByLabelText("New switch name")).toBeNull();
});
it("cancelling a new switch also cancels its capture", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  fireEvent.click(screen.getByRole("button", { name: "Add switch" }));
  await screen.findByRole("button", { name: "Cancel capture" });
  event({ ...current, capture: { active: false, key: "Enter", error: null } });
  await screen.findByText("Enter");
  fireEvent.click(screen.getByRole("button", { name: "Cancel new switch" }));
  expect(screen.queryByLabelText("New switch name")).toBeNull();
  expect(current.settings.bindings).toHaveLength(1);
});
it("rejects a learned duplicate without changing the existing switch", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  fireEvent.click(screen.getByRole("button", { name: "Add switch" }));
  await screen.findByRole("button", { name: "Cancel capture" });
  event({ ...current, capture: { active: false, key: "Space", error: null } });
  await screen.findByText("That key already belongs to another switch.");
  expect(current.settings.bindings).toHaveLength(1);
});
it("reorders hold actions and waits for pending switch saves before enabling", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
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
  expect(screen.getByText("Select · Hold: Stop scanning, Next")).toBeTruthy();
});
it("shows hold timing computed from the interval", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
  expect(
    screen.getByText("Hold 1s for Next, 2s for Stop scanning. Release to run the action shown."),
  ).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "2s" }));
  await screen.findByText("Hold 2s for Next, 4s for Stop scanning. Release to run the action shown.");
  await screen.findByText(/Holding any switch for 8s disables switch control/);
});
it("preserves failed switch edits and prevents enable until retry succeeds", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
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
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
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
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
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
it("holds focus in a modal and swallows keys while learning", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
  fireEvent.click(
    screen.getByRole("button", { name: "Learn another key for Head switch" }),
  );
  const dialog = await screen.findByRole("dialog", {
    name: "Press and release your switch",
  });
  expect(document.activeElement).toBe(dialog);
  expect(screen.getByText(/Learning the key for Head switch/)).toBeTruthy();
  const up = new KeyboardEvent("keyup", { key: " ", bubbles: true, cancelable: true });
  screen.getByRole("button", { name: "Cancel capture" }).dispatchEvent(up);
  expect(up.defaultPrevented).toBe(true);
  expect(mocks.invoke).not.toHaveBeenCalledWith("cancel_switch_capture");
  event({ ...current, capture: { active: false, key: "F3", error: null } });
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  await waitFor(() => expect(current.settings.bindings[0].key).toBe("F3"));
});
it("keeps a refused capture's error on its own row and releases the target", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
  mocks.invoke.mockImplementationOnce(async () => {
    throw "Focus Switchify PC before learning a switch.";
  });
  fireEvent.click(
    screen.getByRole("button", { name: "Learn another key for Head switch" }),
  );
  await screen.findByText("Focus Switchify PC before learning a switch.");
  expect(screen.queryByRole("dialog")).toBeNull();
  // A later view carrying a remembered key must not land on the refused row.
  event({ ...current, capture: { active: false, key: "F9", error: null } });
  await waitFor(() => expect(mocks.invoke.mock.calls.filter(([c]) => c === "save_switches")).toHaveLength(0));
  expect(current.settings.bindings[0].key).toBe("Space");
  fireEvent.click(screen.getByRole("button", { name: "Close Head switch" }));
  fireEvent.click(screen.getByRole("button", { name: "Add switch" }));
  await screen.findByRole("dialog");
  expect(screen.queryByText("Focus Switchify PC before learning a switch.")).toBeNull();
});
it("returns focus to the row after Done and to Add switch after saving a new one", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
  fireEvent.click(screen.getByRole("button", { name: "Done" }));
  expect(document.activeElement).toBe(screen.getByRole("button", { name: "Edit Head switch" }));
  fireEvent.click(screen.getByRole("button", { name: "Add switch" }));
  await screen.findByRole("dialog");
  event({ ...current, capture: { active: false, key: "Enter", error: null } });
  await screen.findByText("Enter");
  fireEvent.change(screen.getByLabelText("New switch name"), { target: { value: "Foot" } });
  fireEvent.click(screen.getByRole("button", { name: "Save switch" }));
  await waitFor(() => expect(current.settings.bindings).toHaveLength(2));
  expect(document.activeElement).toBe(screen.getByRole("button", { name: "Add switch" }));
});
it("rejects a duplicate learned onto an existing switch and says so on that row", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  fireEvent.click(screen.getByRole("button", { name: "Add switch" }));
  await screen.findByRole("dialog");
  current = { ...current, capture: { active: false, key: "Enter", error: null } };
  event(current);
  await screen.findByText("Enter");
  fireEvent.change(screen.getByLabelText("New switch name"), { target: { value: "Foot" } });
  fireEvent.click(screen.getByRole("button", { name: "Save switch" }));
  await waitFor(() => expect(current.settings.bindings).toHaveLength(2));
  fireEvent.click(await screen.findByRole("button", { name: "Edit Foot" }));
  fireEvent.click(screen.getByRole("button", { name: "Learn another key for Foot" }));
  await screen.findByRole("dialog");
  event({ ...current, capture: { active: false, key: "Space", error: null } });
  await screen.findByText("That key already belongs to another switch.");
  expect(current.settings.bindings[1].key).toBe("Enter");
  expect(screen.getByLabelText("Learn another key for Foot").getAttribute("aria-describedby")).toBeTruthy();
});
it("announces an unavailable key in the collapsed row", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  event({ ...current, unavailableKeys: ["Space"] });
  await screen.findByText(/unavailable on this computer/);
});
it("keeps focus in the draft when another switch is removed while adding", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  fireEvent.click(screen.getByRole("button", { name: "Add switch" }));
  await screen.findByRole("dialog");
  current = { ...current, capture: { active: false, key: "Enter", error: null } };
  event(current);
  await screen.findByText("Enter");
  fireEvent.click(screen.getByRole("button", { name: "Remove Head switch" }));
  await waitFor(() => expect(current.settings.bindings).toHaveLength(0));
  expect(document.activeElement).toBe(screen.getByLabelText("New switch name"));
});
