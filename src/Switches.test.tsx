import { useLayoutEffect, type ReactNode } from "react";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { beforeEach, afterEach, it, expect, vi } from "vitest";
import { actionLabel, actions, useSwitches, type SwitchState } from "./scanning/useSwitches";
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
function Shell({ visible = true, mobileConnected = false }: { visible?: boolean; mobileConnected?: boolean }) {
  const switches = useSwitches();
  const scanning = useScanning();
  return (
    <>
      {visible && <SwitchesSection controller={switches} mobileConnected={mobileConnected} />}
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
      case "list_custom_panels":
        return [{ id: "greetings", name: "Greetings", version: 1, description: "", rows: [] }];
      case "list_builtin_panels": case "list_panel_extras": return [];
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
const revealMobile = async () => {
  const button = await screen.findByRole("button", { name: "Mobile switches" });
  if (button.getAttribute("aria-expanded") !== "true") fireEvent.click(button);
};
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
  // Choosing the second hold opens its panel, which moves it earlier.
  fireEvent.click(screen.getByRole("button", { name: "Hold 2s: Stop scanning" }));
  expect(screen.getByRole("button", { name: "Hold 2s: Stop scanning" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("button", { name: "Move hold action 2 later" })).toBeDisabled();
  fireEvent.click(
    screen.getByRole("button", { name: "Move hold action 2 earlier" }),
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
it("edits hold actions from a panel opened by choosing their tile", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
  expect(screen.queryByRole("group", { name: /^Hold action \d/ })).toBeNull();

  // Choosing a tile opens its panel; choosing it again closes it.
  const first = screen.getByRole("button", { name: "Hold 1s: Next" });
  fireEvent.click(first);
  const panel = screen.getByRole("group", { name: "Hold action 1: hold 1s" });
  expect(within(panel).getByText("Hold 1s: Next")).toBeTruthy();
  expect(within(panel).getByRole("button", { name: "Move hold action 1 earlier" })).toBeDisabled();
  fireEvent.click(first);
  expect(screen.queryByRole("group", { name: "Hold action 1: hold 1s" })).toBeNull();

  // Close puts focus back on the tile.
  fireEvent.click(first);
  fireEvent.click(within(screen.getByRole("group", { name: "Hold action 1: hold 1s" })).getByRole("button", { name: "Close" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Hold 1s: Next" })).toHaveFocus());

  // A new hold opens ready to change.
  fireEvent.click(screen.getByRole("button", { name: "Add hold action for Head switch" }));
  await waitFor(() => expect(current.settings.bindings[0].holdActions).toEqual(["next", "stop", "next"]));
  expect(screen.getByRole("group", { name: "Hold action 3: hold 3s" })).toBeTruthy();
  expect(screen.getByLabelText("Hold action 3 for Head switch")).toHaveValue("next");
  await waitFor(() => expect(screen.getByLabelText("Hold action 3 for Head switch")).toHaveFocus());

  // A move that reaches the end disables its button, so focus goes to the action.
  fireEvent.click(screen.getByRole("button", { name: "Move hold action 3 earlier" }));
  await waitFor(() => expect(current.settings.bindings[0].holdActions).toEqual(["next", "next", "stop"]));
  screen.getByRole("button", { name: "Move hold action 2 earlier" }).focus();
  fireEvent.click(screen.getByRole("button", { name: "Move hold action 2 earlier" }));
  await waitFor(() => expect(screen.getByLabelText("Hold action 1 for Head switch")).toHaveFocus());
  expect(screen.getByRole("button", { name: "Move hold action 1 earlier" })).toBeDisabled();
  fireEvent.click(screen.getByRole("button", { name: "Move hold action 1 later" }));
  fireEvent.click(screen.getByRole("button", { name: "Move hold action 2 later" }));
  await waitFor(() => expect(current.settings.bindings[0].holdActions).toEqual(["next", "stop", "next"]));

  // Removing it closes the panel and leaves focus on the tiles.
  fireEvent.click(screen.getByRole("button", { name: "Remove hold action 3" }));
  await waitFor(() => expect(current.settings.bindings[0].holdActions).toEqual(["next", "stop"]));
  expect(screen.queryByRole("group", { name: /^Hold action 3/ })).toBeNull();
  await waitFor(() => expect(screen.getByRole("button", { name: "Add hold action for Head switch" })).toHaveFocus());
});

it("keeps the switch demonstrations behind Show me how", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
  const how = screen.getByRole("button", { name: "Show me how" });
  const editor = how.closest("fieldset")!;
  expect(how).toHaveAttribute("aria-expanded", "false");
  expect(within(editor).queryByRole("button", { name: /^Show me how: / })).toBeNull();
  fireEvent.click(how);
  expect(within(editor).getByRole("button", { name: "Hide how-to" })).toHaveAttribute("aria-expanded", "true");
  expect(within(editor).getAllByRole("button", { name: /^Show me how: / })).toHaveLength(2);
});

it("shows hold timing computed from the interval", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
  const holds = screen.getByRole("list", { name: "Hold" });
  expect(within(holds).getAllByRole("button").map((tile) => tile.getAttribute("aria-label"))).toEqual([
    "Hold 1s: Next",
    "Hold 2s: Stop scanning",
    "Add hold action for Head switch",
  ]);
  expect(within(holds).getByRole("button", { name: "Add hold action for Head switch" })).toHaveTextContent("+ Add a hold for 3s");
  fireEvent.click(screen.getAllByRole("button", { name: "More options" })[0]);
  const interval = screen.getByRole("group", { name: "Hold action interval" });
  expect(within(interval).getByRole("status", { name: "Hold action interval" })).toHaveTextContent("1 s");
  for (let i = 0; i < 20; i++) {
    fireEvent.click(within(interval).getByRole("button", { name: "Increase hold action interval by 0.05 seconds" }));
    await waitFor(() => expect(current.settings.holdIntervalMs).toBe(1050 + i * 50));
  }
  expect(within(interval).getByRole("status", { name: "Hold action interval" })).toHaveTextContent("2 s");
  await screen.findByRole("button", { name: "Hold 2s: Next" });
  expect(screen.getByRole("button", { name: "Hold 4s: Stop scanning" })).toBeTruthy();
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
  expect(screen.getByText(/Learning your switch for Head switch/)).toBeTruthy();
  const up = new KeyboardEvent("keyup", { key: " ", bubbles: true, cancelable: true });
  screen.getByRole("button", { name: "Cancel capture" }).dispatchEvent(up);
  expect(up.defaultPrevented).toBe(true);
  expect(mocks.invoke).not.toHaveBeenCalledWith("cancel_switch_capture");
  event({ ...current, capture: { active: false, key: "F3", error: null } });
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  await waitFor(() => expect(current.settings.bindings[0].key).toBe("F3"));
});
it.each(["learned", "cancelled"])("returns focus to Learn key after capture is %s", async (outcome) => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  open("Head switch");
  const learn = screen.getByRole("button", { name: "Learn another key for Head switch" });
  learn.focus(); fireEvent.click(learn);
  await screen.findByRole("dialog", { name: "Press and release your switch" });
  expect(learn).toBeDisabled();
  current = { ...current, capture: { active: false, key: outcome === "learned" ? "F3" : null, error: null } };
  await act(async () => event(current));
  await waitFor(() => { expect(learn).toBeEnabled(); expect(learn).toHaveFocus(); });
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
  // The backend reports the finished capture from then on, including in the save reply.
  current = { ...current, capture: { active: false, key: "Enter", error: null } };
  event(current);
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
  await revealMobile();
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
  await revealMobile();
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
it("removes a remote switch and surfaces a failed remote save with retry", async () => {
  render(<Shell />);
  await revealMobile();
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
  if (name.startsWith("Remote")) await revealMobile();
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
  await revealMobile();
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
  if (name.startsWith("Remote")) await revealMobile();
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

it("hands remote draft focus to the committed input before passive work can run", async () => {
  const committedFocus: (Element | null)[] = [];
  function CommitProbe({ children, revision }: { children: ReactNode; revision: number }) {
    useLayoutEffect(() => { if (revision) committedFocus.push(document.activeElement); }, [revision]);
    return children;
  }
  const view = render(<CommitProbe revision={0}><Shell /></CommitProbe>);
  await revealMobile();
  await screen.findByRole("heading", { name: "Remote switch 1" });
  const add = screen.getByRole("button", { name: "Add remote switch" });
  add.focus();
  act(() => {
    fireEvent.click(add);
    // A parent update can commit alongside asynchronous backend state. Observe
    // focus at that commit, before any pending passive effects get a turn.
    view.rerender(<CommitProbe revision={1}><Shell /></CommitProbe>);
  });
  const name = screen.getByRole("textbox", { name: "New switch name" });
  expect(committedFocus).toEqual([name]);
  act(() => {
    fireEvent.click(screen.getByRole("button", { name: "Cancel new switch" }));
    view.rerender(<CommitProbe revision={2}><Shell /></CommitProbe>);
  });
  expect(committedFocus[1]).toBe(screen.getByRole("button", { name: "Add switch" }));
});

it.each(["remove", "discard"])("restores usable focus after confirmed %s with browser inert behavior", async (operation) => {
  const focus = HTMLElement.prototype.focus;
  const spy = vi.spyOn(HTMLElement.prototype, "focus").mockImplementation(function (this: HTMLElement, options?: FocusOptions) {
    // jsdom does not enforce inert; browsers refuse focus inside an inert root.
    if (!this.closest("[inert]")) focus.call(this, options);
  });
  try {
    render(<Shell />);
    await revealMobile();
    await screen.findByRole("heading", { name: "Remote switch 1" });
    if (operation === "remove") {
      fireEvent.click(screen.getByRole("button", { name: "Remove Head switch" }));
    } else {
      fireEvent.click(screen.getByRole("button", { name: "Add remote switch" }));
      fireEvent.change(screen.getByRole("textbox", { name: "New switch name" }), { target: { value: "Foot" } });
      fireEvent.click(screen.getByRole("button", { name: "Cancel new switch" }));
    }
    const dialog = screen.getByRole("alertdialog");
    expect(within(dialog).getByRole("button", { name: "Keep switch" })).toHaveFocus();
    fireEvent.click(within(dialog).getByRole("button", { name: operation === "remove" ? "Remove switch" : "Discard switch" }));
    await waitFor(() => expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: "Add switch" })).toHaveFocus();
  } finally { spy.mockRestore(); }
});

it("separates an empty local configuration from the six remote presets", async () => {
  current.settings.bindings = [];
  remote.slots = ["select", "next", "back", "pause", "reverse", "stop", null, null]
    .map((pressAction) => ({ pressAction, holdActions: [] }));
  render(<Shell />);
  await revealMobile();
  await screen.findByRole("heading", { name: "Remote switch 6" });
  expect(screen.getByText(/No local switches configured/)).toBeInTheDocument();
  expect(screen.getByText(/not detected physical switches/)).toBeInTheDocument();
  expect(screen.getByText(/No mobile device connected/)).toBeInTheDocument();
  expect(screen.getAllByText(/Remote forwarding slot/)).toHaveLength(6);
  expect(mocks.invoke.mock.calls.some(([command]) => command.startsWith("save_"))).toBe(false);
});
it("keeps source identity after renaming and separates connection from tested input", async () => {
  const view = render(<Shell />);
  await revealMobile();
  await screen.findByRole("heading", { name: "Remote switch 1" });
  open("Remote switch 1");
  fireEvent.change(screen.getByLabelText("Name for Remote 1"), { target: { value: "Chin" } });
  await waitFor(() => expect(remote.slots[0].name).toBe("Chin"));
  const row = screen.getByRole("heading", { name: "Chin" }).closest("article")!;
  expect(within(row).getByText("Remote forwarding slot 1")).toBeInTheDocument();
  expect(within(row).getByText("Remote 1")).toBeInTheDocument();
  const localRow = screen.getByRole("heading", { name: "Head switch" }).closest("article")!;
  expect(within(localRow).getByText("On this computer")).toBeInTheDocument();
  view.rerender(<Shell mobileConnected />);
  expect(screen.getByText(/Mobile device connected. This does not confirm/)).toHaveTextContent("A connection alone does not verify a physical switch press.");
  expect(screen.queryByText(/No mobile device connected/)).not.toBeInTheDocument();
  view.rerender(<Shell />);
  expect(screen.getByText(/No mobile device connected/)).toBeInTheDocument();
});

it("keeps saved local switches and testing visible before mobile assignments", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  expect(screen.getByRole("button", { name: "Add switch" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Test switches" })).toBeVisible();
  expect(screen.queryByRole("heading", { name: "Remote switch 1" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Add remote switch" })).toBeNull();
  await revealMobile();
  expect(screen.getByRole("heading", { name: "Remote switch 1" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Add remote switch" })).toBeVisible();
  expect(mocks.invoke.mock.calls.some(([command]) => command.startsWith("save_"))).toBe(false);
});
it("saves press timing, which starts off", async () => {
  render(<Shell />);
  await screen.findByRole("heading", { name: "Head switch" });
  fireEvent.click(screen.getAllByRole("button", { name: "More options" })[0]);
  const accept = screen.getByRole("group", { name: "Hold before a press counts" });
  expect(within(accept).getByRole("status", { name: "Hold before a press counts" })).toHaveTextContent("Off");
  expect(within(accept).getByRole("button", { name: "Decrease hold before a press counts by 0.05 seconds" })).toBeDisabled();
  for (let i = 0; i < 6; i++) {
    fireEvent.click(within(accept).getByRole("button", { name: "Increase hold before a press counts by 0.05 seconds" }));
    await waitFor(() => expect(current.settings.acceptMs).toBe(50 + i * 50));
  }
  const ignore = screen.getByRole("group", { name: "Ignore presses after a press" });
  for (let i = 0; i < 10; i++) {
    fireEvent.click(within(ignore).getByRole("button", { name: "Increase ignore presses after a press by 0.05 seconds" }));
    await waitFor(() => expect(current.settings.ignoreRepeatMs).toBe(50 + i * 50));
  }
  expect(current.settings.acceptMs).toBe(300);
  expect(within(ignore).getByRole("status", { name: "Ignore presses after a press" })).toHaveTextContent("0.5 s");
  // Hold actions, and the hold that resets the scan, start once a press counts.
  open("Head switch");
  await screen.findByRole("button", { name: "Hold 1.3s: Next" });
  expect(screen.getByRole("button", { name: "Hold 2.3s: Stop scanning" })).toBeTruthy();
  await screen.findByText(/Holding any switch for 4.3s resets the scan/);
});

it("assigns built-in and custom panels to local presses and holds", async () => {
  render(<Shell />);
  fireEvent.click(await screen.findByRole("button", { name: "Edit Head switch" }));
  const press = screen.getByLabelText("Normal action for Head switch");
  await within(press).findByRole("option", { name: "Greetings" });
  fireEvent.change(press, { target: { value: JSON.stringify({ openCustomPanel: "greetings" }) } });
  await waitFor(() => expect(current.settings.bindings[0].pressAction).toEqual({ openCustomPanel: "greetings" }));
  fireEvent.click(screen.getByRole("button", { name: "Hold 1s: Next" }));
  fireEvent.change(screen.getByLabelText("Hold action 1 for Head switch"), { target: { value: JSON.stringify({ openMenu: "editing" }) } });
  await waitFor(() => expect(current.settings.bindings[0].holdActions[0]).toEqual({ openMenu: "editing" }));
  expect(press).toHaveValue(JSON.stringify({ openCustomPanel: "greetings" }));
});

it("groups the switch actions under headings with every action listed once", async () => {
  render(<Shell />);
  fireEvent.click(await screen.findByRole("button", { name: "Edit Head switch" }));
  const press = screen.getByLabelText("Normal action for Head switch");
  await within(press).findByRole("option", { name: "Greetings" });
  const groups = Array.from(press.querySelectorAll("optgroup")).map((group) => [group.label, Array.from(group.querySelectorAll("option")).map((o) => o.textContent)]);
  expect(groups).toEqual([
    ["Scanning", ["Select", "Next", "Previous", "Reverse direction", "Pause / resume", "Stop scanning", "Auto select on / off"]],
    ["Mouse", ["Left click at the pointer", "Move in blocks on / off"]],
    ["Open a panel", ["Keyboard", "Point", "Mouse"]],
    ["Open a menu", ["Home", "More clicks", "Editing", "Apps and windows", "Browser", "Tabs", "Zoom", "Media", "Displays", "Scanning settings"]],
    ["My panels", expect.arrayContaining(["Greetings"])],
  ]);
  const builtIn = Array.from(press.querySelectorAll("option")).map((o) => o.value).filter((v) => !v.startsWith("{"));
  expect(builtIn.sort()).toEqual(Object.keys(actions).sort());
  // Summaries outside the picker keep the full names.
  fireEvent.change(press, { target: { value: JSON.stringify({ openMenu: "scanning" }) } });
  await waitFor(() => expect(current.settings.bindings[0].pressAction).toEqual({ openMenu: "scanning" }));
  expect(actionLabel({ openMenu: "scanning" })).toBe("Open Scanning");
  fireEvent.change(press, { target: { value: "toggleMouseBlocks" } });
  await waitFor(() => expect(current.settings.bindings[0].pressAction).toBe("toggleMouseBlocks"));
});

it("leaves out the My panels heading when there are no custom panels", async () => {
  const base = mocks.invoke.getMockImplementation()!;
  mocks.invoke.mockImplementation(async (command, args) => (command === "list_custom_panels" ? [] : base(command, args)));
  render(<Shell />);
  fireEvent.click(await screen.findByRole("button", { name: "Edit Head switch" }));
  await waitFor(() => expect(mocks.invoke.mock.calls.some(([command]) => command === "list_custom_panels")).toBe(true));
  await act(async () => {});
  const press = screen.getByLabelText("Normal action for Head switch");
  expect(Array.from(press.querySelectorAll("optgroup")).map((g) => g.label)).toEqual(["Scanning", "Mouse", "Open a panel", "Open a menu"]);
});

it("keeps a deleted panel assignment visible for replacement", async () => {
  current.settings.bindings[0].pressAction = { openCustomPanel: "gone" };
  render(<Shell />);
  fireEvent.click(await screen.findByRole("button", { name: "Edit Head switch" }));
  const press = screen.getByLabelText("Normal action for Head switch");
  expect((within(press).getByRole("option", { name: "Custom panel unavailable" }) as HTMLOptionElement).selected).toBe(true);
  fireEvent.change(press, { target: { value: "select" } });
  await waitFor(() => expect(current.settings.bindings[0].pressAction).toBe("select"));
});

it("keeps a switch assigned to the retired More menu, which opens Home", async () => {
  current.settings.bindings[0].pressAction = { openMenu: "more" };
  render(<Shell />);
  fireEvent.click(await screen.findByRole("button", { name: "Edit Head switch" }));
  const press = screen.getByLabelText("Normal action for Head switch");
  const kept = within(press).getAllByRole("option", { name: "Home" });
  expect(kept).toHaveLength(2);
  expect((kept[1] as HTMLOptionElement).selected).toBe(true);
  expect(within(press).queryByRole("option", { name: "More" })).not.toBeInTheDocument();
  expect(within(press).getByRole("option", { name: "More clicks" })).toBeInTheDocument();
});

it("assigns panel actions to forwarded switch presses and holds", async () => {
  render(<Shell />);
  await revealMobile();
  fireEvent.click(screen.getByRole("button", { name: "Edit Remote switch 1" }));
  const press = screen.getByLabelText("Normal action for Remote switch 1");
  await within(press).findByRole("option", { name: "Greetings" });
  fireEvent.change(press, { target: { value: JSON.stringify({ openMenu: "home" }) } });
  await waitFor(() => expect(remote.slots[0].pressAction).toEqual({ openMenu: "home" }));
  fireEvent.click(screen.getByRole("button", { name: "Add hold action for Remote switch 1" }));
  await waitFor(() => expect(remote.slots[0].holdActions).toHaveLength(1));
  fireEvent.change(screen.getByLabelText("Hold action 1 for Remote switch 1"), { target: { value: JSON.stringify({ openCustomPanel: "greetings" }) } });
  await waitFor(() => expect(remote.slots[0].holdActions[0]).toEqual({ openCustomPanel: "greetings" }));
});
