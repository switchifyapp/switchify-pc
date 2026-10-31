import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { browserState } from "../api";
import { LicenceSection } from "./LicenceSection";
import type { LicenceView } from "../types";
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
import { invoke } from "@tauri-apps/api/core";
const licensed: LicenceView = { configured: true, status: "licensed", daysRemaining: 0, licenceId: "id", message: "Licensed offline.", storageWarning: false };
afterEach(() => { cleanup(); vi.clearAllMocks(); });
it("keeps a valid licence visible when a replacement key is rejected", async () => {
  vi.mocked(invoke).mockRejectedValue("This licence key is invalid.");
  render(<LicenceSection state={{ ...browserState, licence: licensed }} />);
  fireEvent.change(screen.getByLabelText("Licence key"), { target: { value: "invalid-key" } });
  fireEvent.click(screen.getByRole("button", { name: "Activate key" }));
  expect(await screen.findByRole("alert")).toHaveTextContent("This licence key is invalid.");
  expect(screen.getByText("Licensed offline.")).toBeInTheDocument();
});
it("only removes a licence after the local confirmation", async () => {
  vi.mocked(invoke).mockResolvedValue({ ...licensed, status: "expired", licenceId: null, message: "Trial ended." });
  render(<LicenceSection state={{ ...browserState, licence: licensed }} />);
  fireEvent.click(screen.getByRole("button", { name: "Remove licence from this computer" }));
  expect(invoke).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Remove key" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("remove_licence", undefined));
  expect(await screen.findByText("Trial ended.")).toBeInTheDocument();
});
it("does not read the clipboard until Paste and activate is selected", async () => {
  vi.mocked(invoke).mockResolvedValue(licensed);
  render(<LicenceSection state={{ ...browserState, licence: licensed }} />);
  expect(invoke).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "Paste and activate" }));
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("paste_licence", undefined));
});

it("opens the file picker from a button and activates the chosen file", async () => {
  vi.mocked(invoke).mockResolvedValue(licensed);
  render(<LicenceSection state={{ ...browserState, licence: licensed }} />);
  const input = screen.getByLabelText("Licence file");
  const pickFile = vi.spyOn(input, "click");
  fireEvent.click(screen.getByRole("button", { name: "Import licence file" }));
  expect(pickFile).toHaveBeenCalledOnce();
  expect(invoke).not.toHaveBeenCalled();
  fireEvent.change(input, { target: { files: [{ size: 32, text: async () => "test-file-key" }] } });
  await waitFor(() => expect(invoke).toHaveBeenCalledWith("redeem_licence", { key: "test-file-key" }));
});

it("pauses recovery scanning while a keyboard user enters a key", async () => {
  vi.useFakeTimers();
  Object.defineProperty(window, "__TAURI_INTERNALS__", { value: {}, configurable: true });
  try {
    render(<LicenceSection state={{ ...browserState, licence: { ...licensed, status: "expired" } }} />);
    await act(async () => { vi.advanceTimersByTime(1500); });
    expect(screen.getByRole("button", { name: "Paste and activate" })).toHaveFocus();
    const field = screen.getByLabelText("Licence key");
    act(() => field.focus());
    await act(async () => { vi.advanceTimersByTime(3000); });
    expect(field).toHaveFocus();
    expect(invoke).toHaveBeenCalledWith("set_licence_recovery", { active: false });
    fireEvent.change(field, { target: { value: "a manually entered key" } });
    fireEvent.click(screen.getByRole("button", { name: "Resume automatic button scanning" }));
    await act(async () => { vi.advanceTimersByTime(1500); });
    expect(screen.getByRole("button", { name: "Activate key" })).toHaveFocus();
    expect(field).toHaveValue("a manually entered key");
  } finally {
    cleanup();
    Reflect.deleteProperty(window, "__TAURI_INTERNALS__");
    vi.useRealTimers();
  }
});
