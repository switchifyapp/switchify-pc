import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TryItStep } from "./TryItStep";
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
const blocks = (selected: number) => [1, 2, 3, 4, 5, 6].map(n => ({ id: String(n - 1), label: `Block ${n}`, selected: n === selected }));
const view = { active: true, source: "This computer", message: "Use Select to start.", input: "", action: null, completed: 0, lastChoice: null as string | null, automatic: true, blocks: blocks(0) };
let current = view;
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
  current = view;
  invoke.mockReset(); invoke.mockImplementation(async (command: string) => command === "end_switch_practice" ? undefined : current);
});
afterEach(() => { cleanup(); delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; });
const step = (props: Partial<Parameters<typeof TryItStep>[0]> = {}) =>
  render(<TryItStep hasSwitch phone={false} automatic escapeSeconds={4} suspended={false} {...props} />);

describe("setup Try it step", () => {
  it("explains automatic or manual scanning before practice starts", () => {
    step();
    expect(screen.getByText(/The highlight moves by itself/)).toBeInTheDocument();
    cleanup();
    step({ automatic: false });
    expect(screen.getByText(/Next and Previous to move/)).toBeInTheDocument();
  });

  it("runs practice inline with local switches and confirms the goal", async () => {
    step();
    fireEvent.click(screen.getByRole("button", { name: "Start practice" }));
    expect(invoke).toHaveBeenCalledWith("begin_switch_practice", { remote: false });
    expect(await screen.findByRole("list", { name: "Practice blocks" })).toBeInTheDocument();
    // Practice stays inside the guide rather than opening its own dialog.
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    current = { ...view, lastChoice: "Block 2", completed: 1 };
    expect(await screen.findByText("You chose Block 2. Press Select to try again for Block 3.")).toBeInTheDocument();
    current = { ...view, lastChoice: "Block 3", completed: 2 };
    expect(await screen.findByText("You chose Block 3. Your switch is working.")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Stop practice" }));
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("end_switch_practice"));
    // The success outlasts the practice session.
    expect(await screen.findByRole("button", { name: "Start practice" })).toBeInTheDocument();
    expect(screen.getByText("You chose Block 3. Your switch is working.")).toBeInTheDocument();
  });

  it("ends practice when the guide moves on", async () => {
    const { unmount } = step();
    fireEvent.click(screen.getByRole("button", { name: "Start practice" }));
    await screen.findByRole("list", { name: "Practice blocks" });
    invoke.mockClear();
    unmount();
    expect(invoke).toHaveBeenCalledWith("end_switch_practice");
  });

  it("moves focus with practice and says where the highlight is", async () => {
    current = { ...view, blocks: blocks(2) };
    step();
    fireEvent.click(screen.getByRole("button", { name: "Start practice" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Stop practice" })).toHaveFocus());
    expect(screen.getByText("Highlighted: Block 2")).toBeInTheDocument();
    // The goal is in the text, not only in its outline.
    expect(screen.getByText("Block 3").closest("li")).toHaveTextContent("Block 3Goal");
    fireEvent.click(screen.getByRole("button", { name: "Stop practice" }));
    await waitFor(() => expect(screen.getByRole("button", { name: "Start practice" })).toHaveFocus());
    expect(screen.getByText("Practice ended. Choose Start practice to try again.")).toBeInTheDocument();
  });

  it("ends practice when a pairing request suspends the guide", async () => {
    const { rerender } = step();
    fireEvent.click(screen.getByRole("button", { name: "Start practice" }));
    await screen.findByRole("list", { name: "Practice blocks" });
    invoke.mockClear();
    rerender(<TryItStep hasSwitch phone={false} automatic escapeSeconds={4} suspended />);
    await waitFor(() => expect(invoke).toHaveBeenCalledWith("end_switch_practice"));
    expect(screen.getByRole("button", { name: "Start practice" })).toBeDisabled();
  });

  it("shows why practice could not start", async () => {
    invoke.mockImplementation(async (command: string) => { if (command === "begin_switch_practice") throw new Error("Manual scanning starts once switches cover Select, Next and Previous."); });
    step({ automatic: false });
    fireEvent.click(screen.getByRole("button", { name: "Start practice" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Manual scanning starts once switches cover Select, Next and Previous.");
  });

  it("asks for a switch first when none is saved", () => {
    step({ hasSwitch: false });
    expect(screen.queryByRole("button", { name: "Start practice" })).not.toBeInTheDocument();
    expect(screen.getByText(/Add a switch first/)).toBeInTheDocument();
  });

  it("points phone users at PC control instead of practice", () => {
    step({ phone: true });
    expect(screen.getByRole("heading", { name: "Try it from your phone" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Start practice" })).not.toBeInTheDocument();
  });

  it("says how to open Home and stop scanning", () => {
    step({ escapeSeconds: 3 });
    expect(screen.getByText(/To stop scanning, press/)).toHaveTextContent("hold any switch for 3 seconds");
    expect(screen.getByText(/To stop, choose/)).toHaveTextContent("hold any switch for 3 seconds");
  });
});
