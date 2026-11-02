import { render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PanelPicture, picturePalette } from "./PanelPicture";
import type { CustomPanel } from "../types";

const panel: CustomPanel = {
  id: "greetings",
  version: 1,
  name: "Greetings",
  description: "",
  rows: [
    [
      { label: "Copy", action: { type: "command", command: "copy" }, span: 1 },
      { label: "Hello", action: { type: "text", text: "Hello there" }, span: 2 },
    ],
    [{ label: "Back", action: { type: "back" }, span: 1 }],
  ],
};

describe("PanelPicture", () => {
  afterEach(() => { delete (window as { matchMedia?: unknown }).matchMedia; });

  it("draws every row and tile with its span and style", () => {
    render(<PanelPicture panel={panel} theme="dark" />);
    const picture = screen.getByRole("img", { name: "Picture of Greetings: Copy, Hello; Back" });
    const hello = within(picture).getByText("Hello");
    expect(hello).toHaveStyle({ flexGrow: 2 });
    expect(within(picture).getByText("Copy")).toHaveStyle({ flexGrow: 1 });
    expect(within(picture).getByText("Back")).toHaveAttribute("data-toolbar", "true");
    expect(hello).not.toHaveAttribute("data-toolbar");
    expect(within(picture).getByText("Switchify PC")).toBeInTheDocument();
  });

  it("draws a native panel's keys faintly before the rows added below them", () => {
    const fixed = [[{ label: "q", weight: 1 }], [{ label: "Space", weight: 4 }]];
    render(<PanelPicture panel={panel} fixed={fixed} theme="dark" />);
    const picture = screen.getByRole("img", { name: "Picture of Greetings: its keys, then Copy, Hello; Back" });
    const space = within(picture).getByText("Space");
    expect(space).toHaveAttribute("data-fixed", "true");
    expect(space).toHaveStyle({ flexGrow: 4 });
    expect(within(picture).getByText("Hello")).not.toHaveAttribute("data-fixed");
    // The rows sit above the last row of keys, as they do on the panel.
    expect(within(picture).getByText("q").compareDocumentPosition(within(picture).getByText("Copy")) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(within(picture).getByText("Back").compareDocumentPosition(space) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    render(<PanelPicture panel={{ ...panel, rows: [] }} fixed={fixed} theme="dark" />);
    expect(screen.getByRole("img", { name: "Picture of Greetings: its keys only" })).toBeInTheDocument();
  });

  it("draws columns beside the keys, from the top, with lines added for a long column", () => {
    const fixed = [[{ label: "q", weight: 1 }], [{ label: "Space", weight: 4 }]];
    const tile = (label: string) => ({ label, action: { type: "text", text: label } as const, span: 1 });
    const columns = [[tile("A"), tile("B"), tile("C"), tile("D"), tile("E")], [{ ...tile("Wide"), span: 2 }]];
    render(<PanelPicture panel={panel} fixed={fixed} columns={columns} theme="dark" />);
    const picture = screen.getByRole("img", { name: "Picture of Greetings: its keys, then Copy, Hello; Back; columns: A, B, C, D, E; Wide" });
    const rows = picture.querySelectorAll(".panel-picture-row");
    // Two rows of keys and two of tiles, then a fifth line for E.
    expect(rows).toHaveLength(5);
    expect(within(rows[0] as HTMLElement).getByText("A")).toHaveAttribute("data-column", "1");
    expect(within(rows[0] as HTMLElement).getByText("Wide")).toHaveAttribute("data-column", "2");
    expect(within(rows[4] as HTMLElement).getByText("E")).toBeInTheDocument();
    // The second column keeps its width where it has no tile.
    expect(rows[1].querySelectorAll(".panel-picture-spacer")).toHaveLength(1);
  });

  it("follows the chosen panel theme, or the computer when set to system", () => {
    expect(picturePalette("dark").panel).toBe("#141820");
    expect(picturePalette("light").panel).toBe("#f0f2f6");
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({ matches: query === "(prefers-color-scheme: light)" })) as unknown as typeof window.matchMedia;
    expect(picturePalette("system").panel).toBe("#f0f2f6");
    expect(picturePalette(undefined).panel).toBe("#f0f2f6");
  });
});
