import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
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
type RemoteSlotFixture = { pressAction: string | null; holdActions: string[]; name?: string };
const emptySlot: RemoteSlotFixture = { pressAction: null, holdActions: [] };
const initialRemote: { schemaVersion: number; revision: number; slots: RemoteSlotFixture[] } = {
  schemaVersion: 1,
  revision: 1,
  slots: [{ pressAction: "select", holdActions: [] }, ...Array.from({ length: 7 }, () => emptySlot)],
};
let remote: typeof initialRemote;
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
  const scanning = useScanning();
  return (
    <>
      {visible && <SwitchesSection controller={switches} />}
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
  remote = structuredClone(initialRemote);
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    configurable: true,
    value: {},
  });
  mocks.listen.mockReset().mockResolvedValue(vi.fn());
  mocks.invoke.mockReset().mockImplementation(async (command, args) => {
    switch (command) {
      case "set_switch_keyboard_entry":
        current = { ...current, keyboardEntry: args.active };
        return current;
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
      case "get_remote_switches":
        return structuredClone(remote);
      case "save_remote_switches":
        remote = { ...args.config, revision: remote.revision + 1 };
        return remote;
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
  fireEvent.click(screen.getByRole("button", { name: "Discard switch" }));
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
it("reorders hold actions and reflects the saved order in the summary", async () => {
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
  expect(screen.getByText("Saving switches...")).toBeTruthy();
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
  expect(screen.getByText("Select · Hold: Stop scanning, Next")).toBeTruthy();
  expect(screen.getByLabelText("Normal action for Head switch")).toBeEnabled();
});
it("shows hold timing computed from the interval", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
  expect(
    screen.getByText("Hold 1s for Next, 2s for Stop scanning. Release to run the action shown."),
  ).toBeTruthy();
  fireEvent.click(within(screen.getByRole("group", { name: "Hold action interval" })).getByRole("button", { name: "2s" }));
  await screen.findByText("Hold 2s for Next, 4s for Stop scanning. Release to run the action shown.");
  await screen.findByText(/Holding any switch for 8s resets the scan/);
});
it("preserves failed switch edits until a retry succeeds", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
  mocks.invoke.mockRejectedValueOnce("Disk full");
  fireEvent.change(screen.getByLabelText("Name for Space"), {
    target: { value: "New name" },
  });
  await screen.findByText("Disk full");
  expect(screen.getByLabelText("Name for Space")).toHaveValue("New name");
  expect(screen.getByText("Switch assignments have unsaved changes.")).toBeTruthy();
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
  fireEvent.click(screen.getByRole("button", { name: "Remove switch" }));
  await waitFor(() => expect(current.settings.bindings).toHaveLength(0));
  await waitFor(() => expect(document.activeElement).toBe(screen.getByLabelText("New switch name")));
});
it("lists remote switches with local ones and edits them in place", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Remote switch 1" });
  expect(screen.getByText("Remote 1")).toBeTruthy();
  open("Remote switch 1");
  expect(screen.queryByRole("button", { name: /Learn another key/ })).toBeNull();
  fireEvent.change(screen.getByLabelText("Normal action for Remote switch 1"), { target: { value: "next" } });
  await waitFor(() => expect(remote.slots[0].pressAction).toBe("next"));
  fireEvent.change(screen.getByLabelText("Name for Remote 1"), { target: { value: "Chin" } });
  await waitFor(() => expect(remote.slots[0].name).toBe("Chin"));
  await screen.findByRole("heading", { name: "Chin" });
  fireEvent.click(screen.getByRole("button", { name: "Add hold action for Chin" }));
  await waitFor(() => expect(remote.slots[0].holdActions).toEqual(["next"]));
  expect(mocks.invoke.mock.calls.some(([c]) => c === "save_switches")).toBe(false);
});
it("adds a remote switch into the next free slot and can move it", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Remote switch 1" });
  fireEvent.click(screen.getByRole("button", { name: "Add remote switch" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  const slot = screen.getByLabelText("Remote switch number for Remote switch 2");
  expect(slot).toHaveValue("1");
  fireEvent.change(slot, { target: { value: "3" } });
  fireEvent.click(screen.getByRole("button", { name: "Save switch" }));
  await waitFor(() => expect(remote.slots[3].pressAction).toBe("select"));
  expect(remote.slots[1].pressAction).toBeNull();
  await screen.findByRole("heading", { name: "Remote switch 4" });
  open("Remote switch 4");
  fireEvent.change(screen.getByLabelText("Remote switch number for Remote switch 4"), { target: { value: "1" } });
  await waitFor(() => expect(remote.slots[1].pressAction).toBe("select"));
  expect(remote.slots[3].pressAction).toBeNull();
  expect(screen.getByRole("button", { name: "Close Remote switch 2" })).toBeTruthy();
  expect(document.activeElement).toBe(screen.getByRole("button", { name: "Close Remote switch 2" }));
});
it("moves focus into the remote draft and back to a usable Add button", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Remote switch 1" });
  fireEvent.click(screen.getByRole("button", { name: "Add remote switch" }));
  expect(document.activeElement).toBe(screen.getByLabelText("New switch name"));
  fireEvent.click(screen.getByRole("button", { name: "Cancel new switch" }));
  expect(document.activeElement).toBe(screen.getByRole("button", { name: "Add switch" }));
});
it("removes a remote switch and surfaces a failed remote save with retry", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Remote switch 1" });
  mocks.invoke.mockImplementationOnce(async () => {
    throw "Remote save failed";
  });
  fireEvent.click(screen.getByRole("button", { name: "Remove Remote switch 1" }));
  fireEvent.click(screen.getByRole("button", { name: "Remove switch" }));
  await screen.findByText(/Could not remove the switch/);
  expect(remote.slots[0].pressAction).toBe("select");
  expect(screen.getByRole("heading", { name: "Remote switch 1" })).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Remove switch" }));
  await waitFor(() => expect(remote.slots[0].pressAction).toBeNull());
  expect(screen.queryByRole("heading", { name: "Remote switch 1" })).toBeNull();
});


it("pauses capture only by explicit choice, saves mapped characters and resumes after Done", async () => {
  render(<Shell />);
  fireEvent.click(await screen.findByRole("button", { name: "Edit Head switch" }));
  const name = screen.getByRole("textbox", { name: "Name for Space" });
  name.focus();
  expect(mocks.invoke).not.toHaveBeenCalledWith("set_switch_keyboard_entry", expect.anything());
  fireEvent.click(screen.getByRole("button", { name: "Type with keyboard" }));
  await screen.findByText("Keyboard typing is on. Assigned keys type normally; switch scanning is paused.");
  expect(name).toHaveFocus();
  expect(screen.getByRole("button", { name: "Learn another key for Head switch" })).toBeDisabled();
  fireEvent.change(name, { target: { value: "Big blue switch" } });
  await waitFor(() => expect(current.settings.bindings[0].name).toBe("Big blue switch"));
  fireEvent.click(screen.getByRole("button", { name: "Done" }));
  await waitFor(() => expect(current.keyboardEntry).toBe(false));
});

it("releases keyboard entry when navigating away, even while entry is pending", async () => {
  let finish: (() => void) | undefined;
  const invoke = mocks.invoke.getMockImplementation()!;
  mocks.invoke.mockImplementation((command, args) => command === "set_switch_keyboard_entry" && args.active
    ? new Promise((resolve) => { finish = () => { current = { ...current, keyboardEntry: true }; resolve(current); }; })
    : invoke(command, args));
  const view = render(<Shell />);
  fireEvent.click(await screen.findByRole("button", { name: "Edit Head switch" }));
  fireEvent.click(screen.getByRole("button", { name: "Type with keyboard" }));
  await waitFor(() => expect(finish).toBeDefined());
  view.rerender(<Shell visible={false} />);
  await act(async () => finish!());
  await waitFor(() => expect(current.keyboardEntry).toBe(false));
  expect(mocks.invoke).toHaveBeenCalledWith("set_switch_keyboard_entry", { active: false });
});

it("keeps the name draft and normal switch control if keyboard entry is refused", async () => {
  const invoke = mocks.invoke.getMockImplementation()!;
  mocks.invoke.mockImplementation((command, args) => command === "set_switch_keyboard_entry" && args.active
    ? Promise.reject("Stop forwarding on Remote before typing with the keyboard.") : invoke(command, args));
  render(<Shell />);
  fireEvent.click(await screen.findByRole("button", { name: "Edit Head switch" }));
  fireEvent.click(screen.getByRole("button", { name: "Type with keyboard" }));
  await screen.findByText("Stop forwarding on Remote before typing with the keyboard.");
  expect(screen.getByRole("textbox", { name: "Name for Space" })).toHaveValue("Head switch");
  expect(screen.getByRole("button", { name: "Learn another key for Head switch" })).toBeEnabled();
});


it("preserves a newly learned key while typing and releases capture ownership on Cancel", async () => {
  render(<Shell />);
  fireEvent.click(await screen.findByRole("button", { name: "Add switch" }));
  await screen.findByRole("button", { name: "Cancel capture" });
  current = { ...current, capture: { active: false, key: "Enter", error: null } };
  event(current);
  fireEvent.click(await screen.findByRole("button", { name: "Type with keyboard" }));
  await screen.findByRole("button", { name: "Resume switch control" });
  fireEvent.change(screen.getByRole("textbox", { name: "New switch name" }), { target: { value: "Second switch" } });
  expect(screen.getByText("Enter", { selector: "kbd" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Cancel new switch" }));
  expect(current.keyboardEntry).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Discard switch" }));
  await waitFor(() => expect(current.keyboardEntry).toBe(false));
  expect(current.settings.bindings).toHaveLength(1);
});

it("offers recovery when returning to Switches with keyboard typing still active", async () => {
  current.keyboardEntry = true;
  render(<Shell />);
  fireEvent.click(await screen.findByRole("button", { name: "Resume switch control" }));
  await waitFor(() => expect(current.keyboardEntry).toBe(false));
});


it.each([["Remote switch 1", 0], ["Remote switch 1", 1], ["Head switch", 0], ["Head switch", 1]] as const)("releases keyboard entry when %s is removed using control %s", async (name, index) => {
  render(<Shell />);
  fireEvent.click(await screen.findByRole("button", { name: `Edit ${name}` }));
  fireEvent.click(screen.getByRole("button", { name: "Type with keyboard" }));
  await screen.findByRole("button", { name: "Resume switch control" });
  fireEvent.click(screen.getAllByRole("button", { name: `Remove ${name}` })[index]);
  expect(current.keyboardEntry).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Remove switch" }));
  await waitFor(() => expect(current.keyboardEntry).toBe(false));
  expect(mocks.invoke).toHaveBeenCalledWith("set_switch_keyboard_entry", { active: false });
  expect(screen.queryByRole("heading", { name })).not.toBeInTheDocument();
});
it("keeps an edited local draft and restores focus after cancelling discard", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  fireEvent.click(screen.getByRole("button", { name: "Add switch" }));
  await screen.findByRole("button", { name: "Cancel capture" });
  event({ ...current, capture: { active: false, key: "Enter", error: null } });
  fireEvent.change(await screen.findByLabelText("New switch name"), { target: { value: "Foot" } });
  const cancel = screen.getByRole("button", { name: "Cancel new switch" });
  cancel.focus();
  fireEvent.click(cancel);
  const dialog = screen.getByRole("alertdialog", { name: "Discard this new switch?" });
  const keep = within(dialog).getByRole("button", { name: "Keep switch" });
  expect(keep).toHaveFocus();
  fireEvent.keyDown(keep, { key: "Tab", shiftKey: true });
  expect(within(dialog).getByRole("button", { name: "Discard switch" })).toHaveFocus();
  fireEvent.keyDown(dialog, { key: "Escape" });
  await waitFor(() => expect(cancel).toHaveFocus());
  expect(screen.getByLabelText("New switch name")).toHaveValue("Foot");
});
it("requires discard after changing a remote draft action, while untouched cancellation is immediate", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Remote switch 1" });
  fireEvent.click(screen.getByRole("button", { name: "Add remote switch" }));
  fireEvent.change(screen.getByLabelText("New switch action"), { target: { value: "next" } });
  fireEvent.click(screen.getByRole("button", { name: "Cancel new switch" }));
  expect(screen.getByRole("alertdialog")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Discard switch" }));
  expect(remote.slots[1].pressAction).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Add remote switch" }));
  fireEvent.click(screen.getByRole("button", { name: "Cancel new switch" }));
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
  expect(screen.queryByLabelText("New switch name")).not.toBeInTheDocument();
});
it.each(["Head switch", "Remote switch 1"])("protects both removal controls for %s", async (name) => {
  render(<Shell />);
  await screen.findByRole("heading", { name });
  const remove = screen.getByRole("button", { name: `Remove ${name}` });
  remove.focus();
  fireEvent.click(remove);
  expect(screen.getByRole("alertdialog", { name: `Remove ${name}?` })).toBeInTheDocument();
  expect(mocks.invoke.mock.calls.some(([command]) => command.startsWith("save_"))).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Keep switch" }));
  await waitFor(() => expect(remove).toHaveFocus());
  open(name);
  fireEvent.click(screen.getAllByRole("button", { name: `Remove ${name}` })[1]);
  fireEvent.click(screen.getByRole("button", { name: "Remove switch" }));
  await waitFor(() => expect(screen.queryByRole("heading", { name })).not.toBeInTheDocument());
});
it("retains a local assignment when deletion fails and allows keeping it", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  mocks.invoke.mockImplementationOnce(async () => { throw "Disk full"; });
  fireEvent.click(screen.getByRole("button", { name: "Remove Head switch" }));
  fireEvent.click(screen.getByRole("button", { name: "Remove switch" }));
  await screen.findByText(/Could not remove the switch/);
  fireEvent.click(screen.getByRole("button", { name: "Keep switch" }));
  expect(screen.getByRole("heading", { name: "Head switch" })).toBeInTheDocument();
  expect(current.settings.bindings[0]).toEqual(initial.settings.bindings[0]);
});
