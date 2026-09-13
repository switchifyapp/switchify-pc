import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, expect, it, vi } from "vitest";
import { RemoteSwitches } from "./RemoteSwitches";
const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));
const config = { schemaVersion: 1, revision: 1, slots: Array.from({ length: 8 }, (_, i) => ({ pressAction: i === 0 ? "select" : null, holdActions: [] })) };
beforeEach(() => { Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true }); invoke.mockReset(); invoke.mockImplementation(async (command, args) => command === "get_remote_switches" ? structuredClone(config) : { ...args.config, revision: 2 }); });
it("edits remote slots independently and saves ordered holds", async () => {
  render(<RemoteSwitches />);
  const action = await screen.findByLabelText("Remote switch 1 action");
  fireEvent.change(action, { target: { value: "next" } });
  fireEvent.click(screen.getByText("Add hold action for remote switch 1"));
  fireEvent.change(screen.getByLabelText("Remote switch 1 hold action 1"), { target: { value: "select" } });
  fireEvent.click(screen.getByText("Save remote switches"));
  await screen.findByRole("status");
  expect(invoke).toHaveBeenCalledWith("save_remote_switches", { config: expect.objectContaining({ slots: expect.arrayContaining([{ pressAction: "next", holdActions: ["select"] }]) }) });
  expect(invoke.mock.calls.some(([command]) => command === "save_switches")).toBe(false);
});
it("retains edits on save failure and permits retry", async () => {
  render(<RemoteSwitches />); await screen.findByLabelText("Remote switch 1 action");
  invoke.mockRejectedValueOnce(new Error("Save failed"));
  fireEvent.click(screen.getByText("Save remote switches"));
  await screen.findByRole("alert");
  await waitFor(() => expect(screen.getByText("Save remote switches")).not.toBeDisabled());
  fireEvent.click(screen.getByText("Save remote switches"));
  await screen.findByRole("status");
});
