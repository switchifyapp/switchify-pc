import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  useScanning,
  defaultPointScanConfig,
  type PointScanState,
} from "./scanning/useScanning";
import { ScanningSection } from "./settings/ScanningSection";
function more() {
  const button = screen.getByRole("button", { name: "More options" });
  if (button.getAttribute("aria-expanded") !== "true") fireEvent.click(button);
}
function PointScan() {
  const controller = useScanning();
  return <ScanningSection controller={controller} />;
}

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
// The backend decides whether scanning runs; the panel only reports its message.
const initial: PointScanState = {
  config: defaultPointScanConfig,
  enabled: false,
  phase: "idle",
  paused: false,
  supported: true,
  message: "Scanning starts once a switch has the Select action.",
};
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", {
    configurable: true,
    value: {},
  });
  mocks.listen.mockReset().mockResolvedValue(vi.fn());
  mocks.invoke
    .mockReset()
    .mockImplementation(async (command, args) =>
      command === "get_point_scan" ? initial : { ...initial, ...args },
    );
});
afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

it("shows the backend's reason while scanning is off and offers no toggle", async () => {
  render(<PointScan />);
  await screen.findByText("Scanning starts once a switch has the Select action.");
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise point scanning" }));
  expect(screen.getByRole("button", { name: "Line only" })).toBeEnabled();
});

it("saves settings without an enabled flag and keeps them editable while scanning runs", async () => {
  mocks.invoke.mockImplementation(async (command, args) =>
    command === "get_point_scan"
      ? { ...initial, enabled: true, message: "Ready. Press the select switch to begin." }
      : { ...initial, enabled: true, ...args },
  );
  render(<PointScan />);
  await screen.findByText("Ready to begin.");
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise point scanning" }));
  fireEvent.click(screen.getByRole("button", { name: "Grid then line" }));
  await waitFor(() =>
    expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", {
      config: { ...defaultPointScanConfig, mode: "grid" },
    }),
  );
  expect(screen.getByLabelText("Grid size")).toHaveValue("4");
  expect(screen.getByText(/Assign switch actions in the Switches page/)).toBeInTheDocument();
});

it("reports the scan phase while enabled", async () => {
  mocks.invoke.mockImplementation(async () => ({
    ...initial,
    enabled: true,
    phase: "x",
    paused: true,
  }));
  render(<PointScan />);
  await screen.findByText("Paused. Choose the horizontal position.");
});

it("keeps newer edits across old events", async () => {
  let resolveSave!: (value: PointScanState) => void;
  render(<PointScan />);
  await screen.findByText(initial.message);
  mocks.invoke.mockImplementationOnce(
    () =>
      new Promise<PointScanState>((resolve) => {
        resolveSave = resolve;
      }),
  );
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise point scanning" }));
  fireEvent.click(screen.getByRole("button", { name: "Grid then line" }));
  await waitFor(() => expect(resolveSave).toBeTypeOf("function"));
  fireEvent.change(screen.getByLabelText("Grid size"), {
    target: { value: "7" },
  });
  const event = mocks.listen.mock.calls[0][1];
  act(() => event({ payload: initial }));
  expect(screen.getByLabelText("Grid size")).toHaveValue("7");
  await act(async () =>
    resolveSave({ ...initial, config: { ...initial.config, mode: "grid" } }),
  );
  await waitFor(() =>
    expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", {
      config: { ...initial.config, mode: "grid", gridSize: 7 },
    }),
  );
  await screen.findByText("Saved automatically.");
});

it("retains failed edits and offers a retry", async () => {
  render(<PointScan />);
  await screen.findByText(initial.message);
  mocks.invoke.mockRejectedValueOnce("Disk is full.");
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise point scanning" }));
  fireEvent.click(screen.getByRole("button", { name: "Grid then line" }));
  await screen.findByText("Disk is full.");
  expect(
    screen.getByRole("button", { name: "Grid then line" }),
  ).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByText("Scanning settings have unsaved changes.")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Retry save" }));
  await screen.findByText("Saved automatically.");
});

it("keeps pending saves when its settings panel unmounts", async () => {
  function Shell({ visible }: { visible: boolean }) {
    const controller = useScanning();
    return visible ? (
      <ScanningSection controller={controller} />
    ) : (
      <p>Another view</p>
    );
  }
  let resolveSave!: (value: PointScanState) => void;
  const view = render(<Shell visible />);
  await screen.findByText(initial.message);
  mocks.invoke.mockImplementationOnce(
    () =>
      new Promise<PointScanState>((resolve) => {
        resolveSave = resolve;
      }),
  );
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise point scanning" }));
  fireEvent.click(screen.getByRole("button", { name: "Grid then line" }));
  await waitFor(() => expect(resolveSave).toBeTypeOf("function"));
  view.rerender(<Shell visible={false} />);
  await act(async () =>
    resolveSave({ ...initial, config: { ...initial.config, mode: "grid" } }),
  );
  view.rerender(<Shell visible />);
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise point scanning" }));
  expect(
    screen.getByRole("button", { name: "Grid then line" }),
  ).toHaveAttribute("aria-pressed", "true");
});

it("keeps a newer runtime event when an older save response arrives", async () => {
  render(<PointScan />);
  await screen.findByText(initial.message);
  let resolveSave!: (value: PointScanState) => void;
  mocks.invoke.mockImplementationOnce(
    () =>
      new Promise<PointScanState>((resolve) => {
        resolveSave = resolve;
      }),
  );
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise point scanning" }));
  fireEvent.click(screen.getByRole("button", { name: "Grid then line" }));
  await waitFor(() => expect(resolveSave).toBeTypeOf("function"));
  act(() =>
    mocks.listen.mock.calls[0][1]({
      payload: {
        ...initial,
        message: "Mobile device connected. Local scanning stopped.",
      },
    }),
  );
  await act(async () =>
    resolveSave({ ...initial, enabled: true, message: "Ready." }),
  );
  expect(
    screen.getByText("Mobile device connected. Local scanning stopped."),
  ).toBeInTheDocument();
});

it("explains the selectable row escape phase", async () => {
  mocks.invoke.mockResolvedValue({ ...initial, enabled: true, phase: "rowEscape" });
  render(<PointScan />);
  expect(await screen.findByText("Select to return to rows.")).toBeInTheDocument();
});

it.each([
  ["autoSelecting", "Waiting to click. Press a switch for the action menu."],
  ["menu", "Choose an action at the selected point."],
  ["menuSuspended", "Select to resume the action menu."],
  ["dragDestination", "Choose drag destination."],
  ["dragConfirmation", "Confirm drag."],
  ["executing", "Performing drag."],
  ["keyboard", "Choose a keyboard row, then a key."],
  ["keyboardSuspended", "Select to resume the keyboard."],
  ["keyboardOpening", "Opening the keyboard."],
])("explains the %s workflow phase", async (phase, message) => {
  mocks.invoke.mockResolvedValue({ ...initial, enabled: true, phase });
  render(<PointScan />);
  expect(await screen.findByText(message)).toBeInTheDocument();
});

it("saves scanner colour and updates the sample", async () => {
  render(<PointScan />);
  await screen.findByText(initial.message);
  more();
  expect(screen.getByRole("radio", { name: "Blue" })).toBeChecked();
  more();
  fireEvent.click(screen.getByRole("radio", { name: "Green" }));
  expect(screen.getByRole("img", { name: "green scanner highlight sample" })).toBeInTheDocument();
  await waitFor(() => expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", {
    config: { ...defaultPointScanConfig, scannerColor: "green" },
  }));
});

it("retains a failed colour selection for retry", async () => {
  render(<PointScan />);
  await screen.findByText(initial.message);
  mocks.invoke.mockRejectedValueOnce(new Error("Cannot save colour"));
  more();
  fireEvent.click(screen.getByRole("radio", { name: "White" }));
  expect(await screen.findByRole("button", { name: "Retry save" })).toBeEnabled();
  expect(screen.getByRole("radio", { name: "White" })).toBeChecked();
  fireEvent.click(screen.getByRole("button", { name: "Retry save" }));
  await waitFor(() => expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", {
    config: { ...defaultPointScanConfig, scannerColor: "white" },
  }));
});

it("saves auto selection separately from scan movement and validates its delay", async () => {
  render(<PointScan />);
  await screen.findByText(initial.message);
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise point scanning" }));
  expect(screen.queryByRole("spinbutton", { name: "Auto select delay (seconds)" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Click options" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Auto select" }));
  const delay = screen.getByRole("spinbutton", { name: "Auto select delay (seconds)" });
  expect(delay).toHaveValue(1);
  fireEvent.change(delay, { target: { value: "0.5" } });
  await waitFor(() => expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", {
    config: { ...defaultPointScanConfig, autoSelectEnabled: true, autoSelectDelayMs: 500 },
  }));
  const calls = mocks.invoke.mock.calls.length;
  fireEvent.change(delay, { target: { value: "0.05" } });
  expect(mocks.invoke).toHaveBeenCalledTimes(calls);
});


it("customises individual settings, restores defaults and returns focus without saving on navigation", async () => {
  render(<PointScan />);
  await screen.findByText(initial.message);
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise keyboard" }));
  expect(mocks.invoke).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("heading", { name: "Keyboard" })).toHaveFocus();
  expect(screen.queryByRole("checkbox", { name: "Auto select" })).not.toBeInTheDocument();
  expect(screen.getByRole("checkbox", { name: "Word prediction" })).toBeEnabled();
  expect(screen.getByRole("checkbox", { name: "Enhanced word prediction" })).not.toBeChecked();
  expect(screen.getByText(/Your typing never leaves this computer/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("checkbox", { name: "Enhanced word prediction" }));
  await waitFor(() => expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", {
    config: expect.objectContaining({ enhancedWordPrediction: true })
  }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Word prediction" }));
  expect(screen.getByRole("checkbox", { name: "Enhanced word prediction" })).toBeDisabled();
  fireEvent.click(screen.getByRole("checkbox", { name: "Word prediction" }));
  mocks.invoke.mockClear();
  expect(screen.getByText(/The scanning keyboard spaces punctuation for you/)).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Reverse" }));
  await waitFor(() => expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", {
    config: expect.objectContaining({ scanPreferences: expect.objectContaining({ keyboard: { direction: "reverse" }, menu: {} }) })
  }));
  fireEvent.click(screen.getByRole("button", { name: "Use default for initial direction" }));
  expect(screen.getByRole("button", { name: "Forward" })).toHaveAttribute("aria-pressed", "true");
  fireEvent.click(screen.getByRole("button", { name: "Back to scanning settings" }));
  expect(screen.getByRole("button", { name: "Customise keyboard" })).toHaveFocus();
  fireEvent.click(screen.getByRole("button", { name: "Reverse" }));
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise keyboard" }));
  expect(screen.getByRole("button", { name: "Reverse" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.queryByRole("button", { name: "Use default for initial direction" })).not.toBeInTheDocument();
});

it("keeps rate values in manual mode and preserves keyboard settings when resetting overrides", async () => {
  render(<PointScan />);
  await screen.findByText(initial.message);
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise keyboard" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Word prediction" }));
  fireEvent.click(screen.getByRole("button", { name: "Decrease auto scan interval by 0.1 seconds" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Automatic scanning" }));
  expect(screen.getByRole("button", { name: "Decrease auto scan interval by 0.1 seconds" })).toBeDisabled();
  expect(screen.getByLabelText("Auto scan rate")).toHaveTextContent("0.9 s");
  fireEvent.click(screen.getByRole("button", { name: "Use default for auto scan rate" }));
  await waitFor(() => expect(screen.getByRole("group", { name: "Auto scan rate setting" })).toHaveFocus());
  expect(screen.getByRole("button", { name: "Decrease auto scan interval by 0.1 seconds" })).toBeDisabled();
  expect(screen.getByLabelText("Auto scan rate")).toHaveTextContent("1 s");
  fireEvent.click(screen.getByRole("button", { name: "Reset scanning overrides" }));
  expect(screen.getByRole("button", { name: "Decrease auto scan interval by 0.1 seconds" })).toBeEnabled();
  expect(screen.getByLabelText("Auto scan rate")).toHaveTextContent("1 s");
  expect(screen.getByRole("checkbox", { name: "Word prediction" })).not.toBeChecked();
  await waitFor(() => expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", {
    config: expect.objectContaining({ wordPrediction: false, scanPreferences: expect.objectContaining({ keyboard: {} }) })
  }));
});

it("restores overview scroll and leaves point-specific values intact on override reset", async () => {
  render(<PointScan />);
  await screen.findByText(initial.message);
  document.documentElement.scrollTop = 380;
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise point scanning" }));
  expect(screen.queryByRole("button", { name: "One item at a time" })).not.toBeInTheDocument();
  expect(screen.queryByRole("checkbox", { name: "Word prediction" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Grid then line" }));
  fireEvent.click(screen.getByRole("button", { name: "Click options" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Auto select" }));
  fireEvent.click(screen.getByRole("button", { name: "Reverse" }));
  fireEvent.click(screen.getByRole("button", { name: "Reset scanning overrides" }));
  expect(screen.getByRole("button", { name: "Grid then line" })).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByRole("checkbox", { name: "Auto select" })).toBeChecked();
  await waitFor(() => expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", {
    config: expect.objectContaining({ mode: "grid", autoSelectEnabled: true, scanPreferences: expect.objectContaining({ point: {} }) })
  }));
  const calls = mocks.invoke.mock.calls.length;
  fireEvent.click(screen.getByRole("button", { name: "Back to scanning settings" }));
  expect(document.documentElement.scrollTop).toBe(380);
  expect(screen.getByRole("button", { name: "Customise point scanning" })).toHaveFocus();
  expect(mocks.invoke).toHaveBeenCalledTimes(calls);
  document.documentElement.scrollTop = 0;
});

it("brings the area header into view with help expanded and restores the overview on Back", async () => {
  const previous = Object.getOwnPropertyDescriptor(HTMLElement.prototype, "scrollIntoView");
  const scrollIntoView = vi.fn();
  Object.defineProperty(HTMLElement.prototype, "scrollIntoView", { configurable: true, value: scrollIntoView });
  try {
    render(<PointScan />);
    await screen.findByText(initial.message);
    fireEvent.click(screen.getByText("How scanning works"));
    expect(screen.getByText("How scanning works").closest("details")).toHaveAttribute("open");
    document.documentElement.scrollTop = 640;
    more();
  fireEvent.click(screen.getByRole("button", { name: "Customise keyboard" }));
    const heading = screen.getByRole("heading", { name: "Keyboard" });
    expect(heading).toHaveFocus();
    expect(scrollIntoView).toHaveBeenCalledExactlyOnceWith({ block: "start", behavior: "instant" });
    expect(scrollIntoView.mock.contexts[0]).toBe(heading.closest("header"));
    expect(document.documentElement.scrollTop).not.toBe(0);
    expect(mocks.invoke).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Back to scanning settings" }));
    expect(screen.getByRole("button", { name: "Customise keyboard" })).toHaveFocus();
    expect(document.documentElement.scrollTop).toBe(640);
    expect(scrollIntoView).toHaveBeenCalledTimes(1);
  } finally {
    document.documentElement.scrollTop = 0;
    if (previous) Object.defineProperty(HTMLElement.prototype, "scrollIntoView", previous);
    else Reflect.deleteProperty(HTMLElement.prototype, "scrollIntoView");
  }
});

it("saves the shared panel pointer-avoidance choice from the mouse and keyboard pages", async () => {
  mocks.invoke.mockImplementation(async (command, args) => command === "get_point_scan" ? initial : { ...initial, config: args.config });
  render(<PointScan />);
  await screen.findByText(initial.message);
  expect(screen.queryByRole("checkbox", { name: "Move away from the pointer" })).not.toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Customise mouse scanning" }));
  expect(screen.getByRole("checkbox", { name: "Move away from the pointer" })).not.toBeChecked();
  fireEvent.click(screen.getByRole("checkbox", { name: "Move away from the pointer" }));
  await waitFor(() => expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", { config: { ...defaultPointScanConfig, panelAvoidsPointer: true } }));
  fireEvent.click(screen.getByRole("button", { name: /Back to scanning settings/ }));
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise keyboard" }));
  expect(screen.getByRole("checkbox", { name: "Move away from the pointer" })).toBeChecked();
});

it("saves the keyboard after-typing choice and preserves it while manual", async () => {
  mocks.invoke.mockImplementation(async (command, args) => command === "get_point_scan" ? initial : { ...initial, config: args.config });
  render(<PointScan />);
  await screen.findByText(initial.message);
  expect(screen.queryByText("After typing")).not.toBeInTheDocument();
  more();
  fireEvent.click(screen.getByRole("button", { name: "Customise keyboard" }));
  expect(screen.getByRole("button", { name: "Continue scanning" })).toHaveAttribute("aria-pressed", "true");
  fireEvent.click(screen.getByRole("button", { name: "Wait for Select" }));
  await waitFor(() => expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", { config: { ...defaultPointScanConfig, keyboardWaitAfterTyping: true } }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Automatic scanning" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Wait for Select" })).toBeDisabled());
  expect(screen.getByRole("button", { name: "Wait for Select" })).toHaveAttribute("aria-pressed", "true");
  fireEvent.click(screen.getByRole("checkbox", { name: "Automatic scanning" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "Wait for Select" })).toBeEnabled());
  expect(screen.getByRole("button", { name: "Wait for Select" })).toHaveAttribute("aria-pressed", "true");
});

it.each([100, 250, 750, 10000])("adjusts and persists the shared interval from %i ms without rounding legacy values", async (rate) => {
  mocks.invoke.mockImplementation(async (command, args) => command === "get_point_scan"
    ? { ...initial, config: { ...initial.config, blockIntervalMs: rate } } : { ...initial, ...args });
  render(<PointScan />);
  await screen.findByText(initial.message);
  const decrease = screen.getByRole("button", { name: "Decrease auto scan interval by 0.1 seconds" });
  const increase = screen.getByRole("button", { name: "Increase auto scan interval by 0.1 seconds" });
  expect(decrease).toHaveProperty("disabled", rate === 100);
  expect(increase).toHaveProperty("disabled", rate === 10000);
  const next = rate === 100 ? 200 : Math.max(100, rate - 100);
  fireEvent.click(rate === 100 ? increase : decrease);
  expect(screen.getByLabelText("Auto scan rate")).toHaveTextContent(`${next / 1000} s`);
  await waitFor(() => expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", {
    config: expect.objectContaining({ blockIntervalMs: next })
  }));
});

it("shows both modes before their settings and keeps advanced panels collapsed", async () => {
  render(<PointScan />);
  await screen.findByText(initial.message);
  expect(screen.getByRole("button", { name: "Customise point scanning" })).toBeVisible();
  expect(screen.getByRole("button", { name: "Customise mouse scanning" })).toBeVisible();
  expect(screen.queryByRole("group", { name: "Method" })).toBeNull();
  expect(screen.getByRole("checkbox", { name: "Automatic scanning" })).toBeVisible();
  expect(screen.queryByRole("radio", { name: "Blue" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Customise keyboard" })).toBeNull();
  more();
  expect(screen.getByRole("radio", { name: "Blue" })).toBeChecked();
  expect(screen.getByRole("button", { name: "Customise keyboard" })).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "More options" }));
  fireEvent.click(screen.getByRole("button", { name: "Customise point scanning" }));
  expect(screen.getByRole("group", { name: "Method" })).toBeVisible();
  expect(screen.getByRole("group", { name: "Line speed" })).toBeVisible();
  expect(mocks.invoke.mock.calls.some(([command]) => command === "configure_point_scan")).toBe(false);
});

it("explains the saved Mouse mode and keeps mode cards keyboard accessible", async () => {
  mocks.invoke.mockImplementation((command) => command === "get_point_scan" ? Promise.resolve({ ...initial, config: { ...defaultPointScanConfig, controlMode: "mouse" } }) : Promise.resolve(initial));
  render(<PointScan />);
  await screen.findByText(initial.message);
  expect(screen.getAllByText("Mouse scanning", { selector: "strong" })).toHaveLength(2);
  const mouse = screen.getByRole("button", { name: "Customise mouse scanning" });
  mouse.focus();
  fireEvent.keyDown(mouse, { key: "Enter" });
  fireEvent.click(mouse);
  expect(screen.getByRole("heading", { name: "Mouse scanning", level: 2 })).toHaveFocus();
  fireEvent.click(screen.getByRole("button", { name: "Back to scanning settings" }));
  expect(screen.getByRole("button", { name: "Customise mouse scanning" })).toHaveFocus();
});
