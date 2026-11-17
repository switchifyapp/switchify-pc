import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PaceStep } from "./PaceStep";
import { defaultPointScanConfig, type PointScanConfig, type ScanningController } from "../scanning/useScanning";
import { defaultScanPreferences } from "../scanning/preferences";

function controller(config: Partial<PointScanConfig> = {}, extra: Partial<ScanningController> = {}): ScanningController {
  const full = { ...defaultPointScanConfig, ...config };
  return { config: full, state: { config: full, supported: true, enabled: false, paused: false, phase: "idle", message: "" }, pending: 0, error: null, unsaved: false, update: vi.fn(), chooseKeyboardLayout: vi.fn(), retry: vi.fn(), ...extra };
}
function reduceMotion(reduce: boolean) {
  Object.defineProperty(window, "matchMedia", { configurable: true, value: (query: string) => ({
    matches: reduce && query.includes("reduce"), media: query, onchange: null,
    addEventListener: vi.fn(), removeEventListener: vi.fn(), addListener: vi.fn(), removeListener: vi.fn(), dispatchEvent: vi.fn(),
  }) as MediaQueryList });
}
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.useRealTimers(); delete (window as unknown as Record<string, unknown>).matchMedia; });

describe("setup pace step", () => {
  it("says which scanners keep their own rate", () => {
    render(<PaceStep scanning={controller({ scanPreferences: { ...defaultScanPreferences, menu: { intervalMs: 1800 }, keyboard: { intervalMs: 1200 } } })} />);
    expect(screen.getByText("The Home menu and the keyboard keep their own scan rate, which a pace here does not change. Change them in Scanning.")).toBeInTheDocument();
  });

  it("shows a failed save with a way to retry", () => {
    const retry = vi.fn();
    render(<PaceStep scanning={controller({}, { error: "Disk full", retry })} />);
    expect(screen.getByRole("alert")).toHaveTextContent("Your pace was not saved. Disk full");
    fireEvent.click(screen.getByRole("button", { name: "Retry save" }));
    expect(retry).toHaveBeenCalled();
  });

  it("names the line speed when only it differs from a pace", () => {
    render(<PaceStep scanning={controller({ blockIntervalMs: 1000, speed: 4 })} />);
    expect(screen.getByText(/1 s on each item, with Line speed Very fast/)).toBeInTheDocument();
  });

  it("explains why the paces are unavailable when scanning is not supported", () => {
    const scanning = controller();
    scanning.state = { ...scanning.state!, supported: false, message: "Open the desktop app to use point scan." };
    render(<PaceStep scanning={scanning} />);
    expect(screen.getByRole("button", { name: "Slow" })).toBeDisabled();
    expect(screen.getByText("Open the desktop app to use point scan.")).toBeInTheDocument();
  });

  it("starts the preview paused when reduced motion is preferred", () => {
    reduceMotion(true);
    render(<PaceStep scanning={controller()} />);
    expect(screen.getByRole("button", { name: "Play preview" })).toBeInTheDocument();
  });

  it("holds the preview while the guide is suspended", () => {
    vi.useFakeTimers();
    const { rerender } = render(<PaceStep scanning={controller()} suspended />);
    const highlighted = () => screen.getAllByRole("listitem").findIndex((item) => item.dataset.selected === "true");
    act(() => { vi.advanceTimersByTime(3000); });
    expect(highlighted()).toBe(0);
    rerender(<PaceStep scanning={controller()} />);
    act(() => { vi.advanceTimersByTime(1000); });
    expect(highlighted()).toBe(1);
  });
});
