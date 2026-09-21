import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SwitchPractice } from "./SwitchPractice";
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
const view = { active: true, source: "Local keyboard", message: "Press Select", input: "", action: null, completed: 0, rectangles: [], tiles: [], label: "" };
beforeEach(() => {
  Object.defineProperty(window, "__TAURI_INTERNALS__", { configurable: true, value: {} });
  invoke.mockReset(); invoke.mockImplementation(async (command: string) => command === "end_switch_practice" ? undefined : view);
});
afterEach(() => { cleanup(); delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__; });
describe("safe switch practice", () => {
  it("uses saved physical input via the backend, then explicitly releases ownership on exit", async () => {
    render(<SwitchPractice />); fireEvent.click(screen.getByRole("button", { name: "Test switches" }));
    const dialog = await screen.findByRole("dialog", { name: "Safe switch practice" });
    expect(invoke).toHaveBeenCalledWith("begin_switch_practice", { remote: false });
    expect(dialog).toHaveFocus();
    expect(screen.getByRole("button", { name: "Start another test" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Exit practice" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(invoke).toHaveBeenCalledWith("end_switch_practice");
    expect(screen.getByRole("button", { name: "Test switches" })).toHaveFocus();
  });
  it("shows disconnected remote errors rather than reporting successful input", async () => {
    invoke.mockRejectedValueOnce(new Error("No remote forwarding session"));
    render(<SwitchPractice />); fireEvent.change(screen.getByLabelText("Test input"), { target: { value: "remote" } });
    fireEvent.click(screen.getByRole("button", { name: "Test switches" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("No remote forwarding session");
    expect(invoke).toHaveBeenCalledWith("begin_switch_practice", { remote: true });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });
  it("retains the dialog and recovery when releasing capture fails", async () => {
    render(<SwitchPractice />); fireEvent.click(screen.getByRole("button", { name: "Test switches" }));
    await screen.findByRole("dialog");
    invoke.mockImplementation(async (command: string) => { if (command === "end_switch_practice") throw new Error("Release failed"); return view; });
    fireEvent.click(screen.getByRole("button", { name: "Exit practice" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Release failed");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });
  it("ends a late successful start after its component has unmounted", async () => {
    let resolve!: (v: typeof view) => void;
    invoke.mockImplementation((command: string) => command === "begin_switch_practice" ? new Promise(r => { resolve = r; }) : Promise.resolve());
    const result = render(<SwitchPractice />); fireEvent.click(screen.getByRole("button", { name: "Test switches" })); result.unmount();
    await act(async () => { resolve(view); });
    expect(invoke.mock.calls.filter(c => c[0] === "end_switch_practice")).toHaveLength(2);
  });
  it("retains safe ownership when a feedback poll fails until explicit exit", async () => {
    invoke.mockImplementation(async (command: string) => {
      if (command === "get_switch_practice") throw new Error("Feedback failed");
      return command === "end_switch_practice" ? undefined : view;
    });
    render(<SwitchPractice />); fireEvent.click(screen.getByRole("button", { name: "Test switches" }));
    await screen.findByRole("dialog");
    expect(await screen.findByRole("alert")).toHaveTextContent("Use Exit practice");
    expect(invoke).not.toHaveBeenCalledWith("end_switch_practice");
    expect(screen.getByRole("dialog")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Exit practice" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(invoke).toHaveBeenCalledWith("end_switch_practice");
  });
  it("ignores a stale failed poll after exit and reopening", async () => {
    let reject!: (reason: Error) => void;
    let first = true;
    invoke.mockImplementation((command: string) => {
      if (command === "get_switch_practice" && first) { first = false; return new Promise((_, r) => { reject = r; }); }
      return Promise.resolve(command === "end_switch_practice" ? undefined : view);
    });
    render(<SwitchPractice />); fireEvent.click(screen.getByRole("button", { name: "Test switches" }));
    await screen.findByRole("dialog"); await waitFor(() => expect(reject).toBeDefined());
    fireEvent.click(screen.getByRole("button", { name: "Exit practice" }));
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: "Test switches" })); await screen.findByRole("dialog");
    await act(async () => { reject(new Error("Old feedback failure")); });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
    expect(invoke.mock.calls.filter(c => c[0] === "end_switch_practice")).toHaveLength(1);
  });
  it("does not allow starting with unsaved settings or without a native backend", () => {
    const { rerender } = render(<SwitchPractice disabled />);
    expect(screen.getByRole("button", { name: "Test switches" })).toBeDisabled();
    delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
    rerender(<SwitchPractice />); expect(screen.getByRole("button", { name: "Test switches" })).toBeDisabled();
  });
});
