import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  isModifierOverlayRoute,
  ModifierOverlay,
  ModifierOverlayView,
  newestModifierSnapshot,
} from "./ModifierOverlay";

const { invokeMock, listenMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));
vi.mock("@tauri-apps/api/event", () => ({ listen: listenMock }));

describe("modifier overlay", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    listenMock.mockReset();
  });

  it("selects only the dedicated overlay route", () => {
    expect(isModifierOverlayRoute("?view=modifier-overlay")).toBe(true);
    expect(isModifierOverlayRoute("?view=settings")).toBe(false);
    expect(isModifierOverlayRoute("")).toBe(false);
  });

  it("renders platform labels in the supplied canonical order", () => {
    render(
      <ModifierOverlayView snapshot={{ revision: 4, labels: ["Control", "Option", "Shift", "Command"] }} />,
    );
    expect(screen.getByRole("status", { name: "Active modifiers" })).toHaveTextContent(
      "ControlOptionShiftCommand",
    );
  });

  it("marks the empty overlay as hidden", () => {
    const { container } = render(
      <ModifierOverlayView snapshot={{ revision: 2, labels: [] }} />,
    );
    expect(container.firstChild).toHaveAttribute("data-empty", "true");
    expect(container.firstChild).toHaveAttribute("aria-hidden", "true");
  });

  it("does not allow an older event to replace current state", () => {
    const current = { revision: 8, labels: ["Shift"] };
    expect(newestModifierSnapshot(current, { revision: 7, labels: [] })).toBe(current);
    expect(newestModifierSnapshot(current, { revision: 9, labels: ["Command"] })).toEqual({
      revision: 9,
      labels: ["Command"],
    });
  });

  it("registers the listener before requesting initial state", async () => {
    const calls: string[] = [];
    listenMock.mockImplementation(async () => {
      calls.push("listen");
      return vi.fn();
    });
    invokeMock.mockImplementation(async (command: string) => {
      calls.push(command);
      if (command === "modifier_overlay_ready") {
        return { revision: 2, labels: [] };
      }
      return undefined;
    });

    render(<ModifierOverlay />);

    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("modifier_overlay_ready"));
    expect(calls.slice(0, 2)).toEqual(["listen", "modifier_overlay_ready"]);
  });

  it("requests presentation only after nonempty content is rendered", async () => {
    listenMock.mockResolvedValue(vi.fn());
    invokeMock.mockImplementation(async (command: string) => {
      if (command === "modifier_overlay_ready") {
        return { revision: 6, labels: ["Ctrl", "Shift"] };
      }
      if (command === "modifier_overlay_present") {
        expect(screen.getByRole("status", { name: "Active modifiers" })).toHaveTextContent(
          "CtrlShift",
        );
      }
      return undefined;
    });

    render(<ModifierOverlay />);

    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("modifier_overlay_present", { revision: 6 });
    });
  });
});
