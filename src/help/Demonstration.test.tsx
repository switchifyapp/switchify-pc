import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Demonstration, DemonstrationProvider } from "./Demonstration";
import { demonstration, type DemonstrationKind } from "./demonstrations";

let reduced = false;
let mediaChange: (() => void) | undefined;
const advance = (ms = 3050) => act(() => { vi.advanceTimersByTime(ms); });
const open = (title: string) => fireEvent.click(screen.getByRole("button", { name: `Show me how: ${title}` }));
const caption = () => document.querySelector(".teaching-caption")!.textContent;

beforeEach(() => {
  vi.useFakeTimers();
  reduced = false;
  vi.stubGlobal("IntersectionObserver", undefined);
  vi.stubGlobal("matchMedia", () => ({ get matches() { return reduced; }, addEventListener: (_: string, cb: () => void) => { mediaChange = cb; }, removeEventListener: vi.fn() }));
});
afterEach(() => { cleanup(); vi.useRealTimers(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });

describe("teaching demonstrations", () => {
  it("plays once, pauses without losing its place, replays and cancels work on collapse", () => {
    render(<Demonstration kind="learn" />);
    expect(document.querySelector("svg")).toBeNull();
    open("Learn a switch key");
    advance(1000);
    fireEvent.click(screen.getByRole("button", { name: "Pause Learn a switch key" }));
    advance(5000);
    expect(caption()).toMatch(/^1 \/ 3/);
    fireEvent.click(screen.getByRole("button", { name: "Play Learn a switch key" }));
    advance(2200);
    expect(caption()).toMatch(/^2 \/ 3/);
    advance();
    fireEvent.click(screen.getByRole("button", { name: "Pause Learn a switch key" }));
    expect(screen.getByRole("button", { name: "Play Learn a switch key" })).toBeEnabled();
    fireEvent.click(screen.getByRole("button", { name: "Play Learn a switch key" }));
    advance();
    expect(caption()).toMatch(/^3 \/ 3/);
    expect(screen.getByRole("button", { name: "Play Learn a switch key" })).toBeDisabled();
    expect(vi.getTimerCount()).toBe(0);
    fireEvent.click(screen.getByRole("button", { name: "Replay Learn a switch key" }));
    expect(caption()).toMatch(/^1 \/ 3/);
    fireEvent.click(screen.getByRole("button", { name: "Hide: Learn a switch key" }));
    expect(document.querySelector("svg")).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
  });

  it("uses manually selected still illustrations for reduced motion, including live preference changes", () => {
    reduced = true;
    render(<Demonstration kind="hold" />);
    open("Press, hold and release");
    expect(vi.getTimerCount()).toBe(0);
    expect(screen.queryByRole("button", { name: /^Pause/ })).toBeNull();
    for (let i = 0; i < 4; i++) fireEvent.click(screen.getByRole("button", { name: /Next illustration/ }));
    expect(caption()).toContain("Release to run it.");
    expect(screen.getByRole("button", { name: /Next illustration/ })).toBeDisabled();
    act(() => { reduced = false; mediaChange?.(); });
    fireEvent.click(screen.getByRole("button", { name: /Replay/ }));
    expect(vi.getTimerCount()).toBeGreaterThan(0);
    act(() => { reduced = true; mediaChange?.(); });
    expect(vi.getTimerCount()).toBe(0);
  });

  it("suspends during capture or pairing and requires explicit play afterward", () => {
    const content = (suspended: boolean) => <DemonstrationProvider platform="windows" suspended={suspended}><Demonstration kind="pair" /></DemonstrationProvider>;
    const view = render(content(false));
    open("Pair a mobile device"); advance(1000);
    view.rerender(content(true));
    expect(vi.getTimerCount()).toBe(0);
    expect(screen.getByRole("button", { name: /Replay/ })).toBeDisabled();
    view.rerender(content(false));
    advance(); expect(caption()).toMatch(/^1 \/ 4/);
    expect(screen.getByRole("button", { name: /Play Pair/ })).toBeEnabled();
  });

  it("stops animation scheduling off-screen and while the document is hidden", () => {
    let observe: (entries: { isIntersecting: boolean }[]) => void = () => {};
    const disconnect = vi.fn();
    vi.stubGlobal("IntersectionObserver", class {
      constructor(cb: typeof observe) { observe = cb; }
      observe() {}
      disconnect = disconnect;
    });
    const view = render(<Demonstration kind="usb" />);
    open("Connect a USB switch");
    expect(vi.getTimerCount()).toBe(0);
    act(() => observe([{ isIntersecting: true }])); advance(1000);
    act(() => observe([{ isIntersecting: false }]));
    expect(vi.getTimerCount()).toBe(0);
    act(() => observe([{ isIntersecting: true }]));
    const hidden = vi.spyOn(document, "hidden", "get").mockReturnValue(true);
    fireEvent(document, new Event("visibilitychange"));
    expect(vi.getTimerCount()).toBe(0);
    hidden.mockReturnValue(false);
    fireEvent(document, new Event("visibilitychange"));
    expect(vi.getTimerCount()).toBeGreaterThan(0);
    view.unmount(); expect(disconnect).toHaveBeenCalled(); expect(vi.getTimerCount()).toBe(0);
  });

  it("keeps SVG definition IDs unique when the same artwork appears twice", () => {
    render(<><Demonstration kind="jack" /><Demonstration kind="jack" /></>);
    for (const button of screen.getAllByRole("button", { name: /Show me how/ })) fireEvent.click(button);
    const ids = [...document.querySelectorAll("svg [id]")].map(el => el.id);
    expect(ids.length).toBeGreaterThan(0);
    expect(new Set(ids).size).toBe(ids.length);
    expect(document.querySelectorAll('svg[aria-hidden="true"][focusable="false"]')).toHaveLength(2);
  });

  it("shows Accessibility teaching only on macOS", () => {
    const content = (platform: "windows" | "macos") => <DemonstrationProvider platform={platform} suspended={false}><Demonstration kind="access" /></DemonstrationProvider>;
    const view = render(content("windows"));
    expect(screen.queryByRole("button")).toBeNull();
    view.rerender(content("macos")); open("Allow Accessibility on macOS");
    fireEvent.click(screen.getByRole("button", { name: "Read steps: Allow Accessibility on macOS" }));
    expect(screen.getByRole("list")).toHaveTextContent("return to the app");
  });

  it.each(["jack", "usb", "interface", "learn", "select", "grid", "line", "hold", "access", "pair", "startup"] as DemonstrationKind[])("renders every illustrated step for %s", kind => {
    reduced = true;
    render(<DemonstrationProvider platform="macos" suspended={false}><Demonstration kind={kind} /></DemonstrationProvider>);
    const { title, steps, captions } = demonstration(kind, "macos");
    expect(steps).toHaveLength(captions.length);
    open(title);
    for (let i = 0; i < steps.length; i++) {
      expect(document.querySelector("svg")).not.toBeNull();
      expect(caption()).toContain(captions[i]);
      if (i < steps.length - 1) fireEvent.click(screen.getByRole("button", { name: /Next illustration/ }));
    }
    if (kind === "pair") expect(document.querySelectorAll(".pair-code-phone, .pair-code-pc")).toHaveLength(2);
  });
});
