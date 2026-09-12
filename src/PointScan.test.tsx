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
const initial: PointScanState = {
  config: defaultPointScanConfig,
  enabled: false,
  phase: "idle",
  paused: false,
  supported: true,
  message: "Point scan is off.",
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
it("enables native point scan and locks its configuration until disabled", async () => {
  render(<PointScan />);
  await screen.findByText("Point scan is off.");
  fireEvent.click(screen.getByRole("button", { name: "Enable point scan" }));
  await screen.findByRole("button", { name: "Disable point scan" });
  expect(mocks.invoke).toHaveBeenCalledWith("configure_point_scan", {
    config: defaultPointScanConfig,
    enabled: true,
  });
  expect(screen.getByRole("button", { name: "Line only" })).toBeDisabled();
});
it("exposes grid settings and directs switch assignments to their own tab", async () => {
  render(<PointScan />);
  await screen.findByText("Point scan is off.");
  fireEvent.click(screen.getByRole("button", { name: "Grid then line" }));
  expect(screen.getByLabelText("Grid size")).toHaveValue("4");
  expect(screen.queryByLabelText("Forward switch")).not.toBeInTheDocument();
  expect(screen.getByText(/Assign switch actions in the Switches tab/)).toBeInTheDocument();
});
it("reports native registration failure without claiming scanning started", async () => {
  render(<PointScan />);
  await screen.findByText("Point scan is off.");
  mocks.invoke.mockRejectedValueOnce("Space is already in use.");
  fireEvent.click(screen.getByRole("button", { name: "Enable point scan" }));
  await waitFor(() =>
    expect(screen.getByRole("alert")).toHaveTextContent(
      "Space is already in use.",
    ),
  );
  expect(
    screen.getByRole("button", { name: "Enable point scan" }),
  ).toBeEnabled();
});

afterEach(() => {
  Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
});

it("keeps newer edits across old events and saves before enabling", async () => {
  let resolveSave!: (value: PointScanState) => void;
  render(<PointScan />);
  await screen.findByText("Point scan is off.");
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
  fireEvent.click(screen.getByRole("button", { name: "Enable point scan" }));
  expect(
    mocks.invoke.mock.calls.filter(([, args]) => args?.enabled),
  ).toHaveLength(0);
  await act(async () =>
    resolveSave({ ...initial, config: { ...initial.config, mode: "grid" } }),
  );
  await screen.findByRole("button", { name: "Disable point scan" });
  expect(mocks.invoke).toHaveBeenLastCalledWith("configure_point_scan", {
    config: { ...initial.config, mode: "grid", gridSize: 7 },
    enabled: true,
  });
});

it("retains failed edits and requires a successful retry before enabling", async () => {
  render(<PointScan />);
  await screen.findByText("Point scan is off.");
  mocks.invoke.mockRejectedValueOnce("Disk is full.");
  fireEvent.click(screen.getByRole("button", { name: "Grid then line" }));
  await screen.findByText("Disk is full.");
  expect(
    screen.getByRole("button", { name: "Grid then line" }),
  ).toHaveAttribute("aria-pressed", "true");
  fireEvent.click(screen.getByRole("button", { name: "Enable point scan" }));
  await screen.findByText(/Save the scanning settings before enabling/);
  expect(
    mocks.invoke.mock.calls.filter(([, args]) => args?.enabled),
  ).toHaveLength(0);
  fireEvent.click(screen.getByRole("button", { name: "Retry save" }));
  await screen.findByText("Scanning settings save automatically.");
  fireEvent.click(screen.getByRole("button", { name: "Enable point scan" }));
  await screen.findByRole("button", { name: "Disable point scan" });
});

it("keeps pending saves and an enabled scan when its settings panel unmounts", async () => {
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
  await screen.findByText("Point scan is off.");
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
  fireEvent.click(screen.getByRole("button", { name: "Enable point scan" }));
  await screen.findByRole("button", { name: "Disable point scan" });
  const calls = mocks.invoke.mock.calls.length;
  view.rerender(<Shell visible={false} />);
  view.rerender(<Shell visible />);
  expect(
    screen.getByRole("button", { name: "Disable point scan" }),
  ).toBeEnabled();
  expect(mocks.invoke).toHaveBeenCalledTimes(calls);
});

it("keeps a newer cancellation event when an older enable response arrives", async () => {
  render(<PointScan />);
  await screen.findByText("Point scan is off.");
  let resolveEnable!: (value: PointScanState) => void;
  mocks.invoke.mockImplementationOnce(
    () =>
      new Promise<PointScanState>((resolve) => {
        resolveEnable = resolve;
      }),
  );
  fireEvent.click(screen.getByRole("button", { name: "Enable point scan" }));
  await waitFor(() => expect(resolveEnable).toBeTypeOf("function"));
  act(() =>
    mocks.listen.mock.calls[0][1]({
      payload: {
        ...initial,
        message: "Android connected. Local scanning stopped.",
      },
    }),
  );
  await act(async () => resolveEnable({ ...initial, enabled: true }));
  expect(
    screen.getByRole("button", { name: "Enable point scan" }),
  ).toBeEnabled();
  expect(
    screen.getByText("Android connected. Local scanning stopped."),
  ).toBeInTheDocument();
});
