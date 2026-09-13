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
  expect(screen.queryByRole("button", { name: /point scan/i })).toBeNull();
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
  fireEvent.click(screen.getByRole("button", { name: "Grid then line" }));
  await waitFor(() =>
    expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", {
      config: { ...defaultPointScanConfig, mode: "grid" },
    }),
  );
  expect(screen.getByLabelText("Grid size")).toHaveValue("4");
  expect(screen.getByText(/Assign switch actions in the Switches tab/)).toBeInTheDocument();
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
  await screen.findByText("Scanning settings save automatically.");
});

it("retains failed edits and offers a retry", async () => {
  render(<PointScan />);
  await screen.findByText(initial.message);
  mocks.invoke.mockRejectedValueOnce("Disk is full.");
  fireEvent.click(screen.getByRole("button", { name: "Grid then line" }));
  await screen.findByText("Disk is full.");
  expect(
    screen.getByRole("button", { name: "Grid then line" }),
  ).toHaveAttribute("aria-pressed", "true");
  expect(screen.getByText("Scanning settings have unsaved changes.")).toBeInTheDocument();
  fireEvent.click(screen.getByRole("button", { name: "Retry save" }));
  await screen.findByText("Scanning settings save automatically.");
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
  fireEvent.click(screen.getByRole("button", { name: "Grid then line" }));
  await waitFor(() => expect(resolveSave).toBeTypeOf("function"));
  view.rerender(<Shell visible={false} />);
  await act(async () =>
    resolveSave({ ...initial, config: { ...initial.config, mode: "grid" } }),
  );
  view.rerender(<Shell visible />);
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
  fireEvent.click(screen.getByRole("button", { name: "Grid then line" }));
  await waitFor(() => expect(resolveSave).toBeTypeOf("function"));
  act(() =>
    mocks.listen.mock.calls[0][1]({
      payload: {
        ...initial,
        message: "Android connected. Local scanning stopped.",
      },
    }),
  );
  await act(async () =>
    resolveSave({ ...initial, enabled: true, message: "Ready." }),
  );
  expect(
    screen.getByText("Android connected. Local scanning stopped."),
  ).toBeInTheDocument();
});

it("explains the selectable row escape phase", async () => {
  mocks.invoke.mockResolvedValue({ ...initial, enabled: true, phase: "rowEscape" });
  render(<PointScan />);
  expect(await screen.findByText("Select to return to rows.")).toBeInTheDocument();
});
