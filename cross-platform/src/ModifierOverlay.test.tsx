import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import {
  isModifierOverlayRoute,
  ModifierOverlayView,
  newestModifierSnapshot,
} from "./ModifierOverlay";

describe("modifier overlay", () => {
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
});
