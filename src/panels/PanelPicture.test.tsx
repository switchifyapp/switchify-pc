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

  it("follows the chosen panel theme, or the computer when set to system", () => {
    expect(picturePalette("dark").panel).toBe("#141820");
    expect(picturePalette("light").panel).toBe("#f0f2f6");
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({ matches: query === "(prefers-color-scheme: light)" })) as unknown as typeof window.matchMedia;
    expect(picturePalette("system").panel).toBe("#f0f2f6");
    expect(picturePalette(undefined).panel).toBe("#f0f2f6");
  });
});
