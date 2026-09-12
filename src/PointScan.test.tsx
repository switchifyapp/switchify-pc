import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { PointScan, defaultPointScanConfig, type PointScanState } from "./PointScan";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), listen: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
const initial: PointScanState = { config: defaultPointScanConfig, enabled: false, phase: "idle", paused: false, supported: true, message: "Point scan is off." };
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
  mocks.listen.mockReset().mockResolvedValue(vi.fn());
  mocks.invoke.mockReset().mockImplementation(async (command, args) => command === "get_point_scan" ? initial : { ...initial, ...args });
});
it("enables native point scan and locks its configuration until disabled", async () => {
  render(<PointScan />);
  await screen.findByText("Point scan is off.");
  fireEvent.click(screen.getByRole("button", { name: "Enable point scan" }));
  await screen.findByRole("button", { name: "Disable point scan" });
  expect(mocks.invoke).toHaveBeenCalledWith("configure_point_scan", { config: defaultPointScanConfig, enabled: true });
  expect(screen.getByLabelText("Mode")).toBeDisabled();
});
it("rejects duplicate switch keys and exposes grid settings", async () => {
  render(<PointScan />); await screen.findByText("Point scan is off.");
  fireEvent.change(screen.getByLabelText("Mode"), { target: { value: "grid" } });
  expect(screen.getByLabelText("Grid size")).toHaveValue("4");
  fireEvent.change(screen.getByLabelText("Forward switch"), { target: { value: "Space" } });
  expect(screen.getByRole("button", { name: "Enable point scan" })).toBeDisabled();
});
it("reports native registration failure without claiming scanning started", async () => {
  render(<PointScan />); await screen.findByText("Point scan is off.");
  mocks.invoke.mockRejectedValueOnce("Space is already in use.");
  fireEvent.click(screen.getByRole("button", { name: "Enable point scan" }));
  await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("Space is already in use."));
  expect(screen.getByRole("button", { name: "Enable point scan" })).toBeEnabled();
});
