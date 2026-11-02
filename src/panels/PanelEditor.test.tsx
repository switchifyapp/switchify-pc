import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PanelEditor } from "./PanelEditor";
import { api } from "../api";
import type { CustomPanel, PanelActionEntry } from "../types";

const catalogue: PanelActionEntry[] = [
  { id: "key", name: "Press keys", category: "typing", categoryLabel: "Typing", keywords: ["shortcut"], action: { type: "key", keys: [] }, parameter: { kind: "keys", min: 1, max: 4 }, needsPoint: false },
  { id: "text", name: "Type text", category: "typing", categoryLabel: "Typing", keywords: ["phrase"], action: { type: "text", text: "" }, parameter: { kind: "text", maxChars: 200 }, needsPoint: false },
  { id: "command.copy", name: "Copy", category: "editing", categoryLabel: "Editing", keywords: ["clipboard"], action: { type: "command", command: "copy" }, needsPoint: false },
  { id: "click.left", name: "Left click", category: "mouse", categoryLabel: "Mouse", keywords: ["tap"], action: { type: "click", button: "left", count: 1 }, needsPoint: true },
  { id: "panel.custom", name: "Open one of my panels", category: "panels", categoryLabel: "Panels and menus", keywords: ["link"], action: { type: "openCustomPanel", id: "" }, parameter: { kind: "panelId" }, needsPoint: false },
  { id: "back", name: "Back", category: "panels", categoryLabel: "Panels and menus", keywords: [], action: { type: "back" }, needsPoint: false },
];

const starter = (): CustomPanel => ({
  id: "greetings", version: 1, name: "Greetings", description: "",
  rows: [
    [{ label: "Keyboard", action: { type: "openPanel", panel: "keyboard" }, span: 1 }, { label: "Mouse", action: { type: "openPanel", panel: "mouse" }, span: 1 }],
    [{ label: "Back", action: { type: "back" }, span: 1 }, { label: "Close", action: { type: "close" }, span: 1 }],
  ],
});
const other: CustomPanel = { id: "board", version: 1, name: "Board", description: "", rows: [[{ label: "Back", action: { type: "back" }, span: 1 }]] };

function mount(panel = starter()) {
  vi.spyOn(api, "panelActions").mockResolvedValue(catalogue);
  const onSave = vi.fn(async (_panel: CustomPanel) => undefined);
  const onClose = vi.fn();
  const onDelete = vi.fn(async () => undefined);
  const onDirtyChange = vi.fn();
  render(<PanelEditor panel={panel} panels={[panel, other]} theme="dark" busy={false} error={null} onClose={onClose} onSave={onSave} onDelete={onDelete} onDirtyChange={onDirtyChange} />);
  return { onSave, onClose, onDelete, onDirtyChange };
}

describe("PanelEditor", () => {
  afterEach(() => vi.restoreAllMocks());

  it("edits a tile's label, width and action from the searchable library", async () => {
    const { onSave, onDirtyChange } = mount();
    expect(screen.getByRole("button", { name: "Save panel" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: /Row 1, tile 1: Keyboard/ }));
    expect(screen.getByRole("heading", { name: "Row 1, tile 1" })).toBeInTheDocument();
    await screen.findByRole("button", { name: "Copy" });

    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "clip" } });
    expect(screen.getAllByRole("group").filter((group) => group.classList.contains("action-group")).map((group) => group.getAttribute("aria-label"))).toEqual(["Editing"]);
    expect(within(screen.getByRole("group", { name: "Editing" })).getAllByRole("button").map((option) => option.textContent)).toEqual(["Copy"]);
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    expect(screen.getByRole("button", { name: "Copy" })).toHaveAttribute("aria-pressed", "true");
    // The label follows the action while it was the default one.
    expect(screen.getByRole("textbox", { name: "Label" })).toHaveValue("Copy");
    expect(screen.getByText("Copy", { selector: ".tile-action-current" })).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox", { name: "Label" }), { target: { value: "Copy it" } });
    fireEvent.change(screen.getByRole("combobox", { name: "Tile width" }), { target: { value: "2" } });
    expect(onDirtyChange).toHaveBeenLastCalledWith(true);

    fireEvent.click(screen.getByRole("button", { name: "Save panel" }));
    await waitFor(() => expect(onSave).toHaveBeenCalled());
    const saved = onSave.mock.calls[0][0];
    expect(saved.rows[0][0]).toEqual({ label: "Copy it", action: { type: "command", command: "copy" }, span: 2 });
    expect(saved.rows[1]).toHaveLength(2);
  });

  it("fills in key, text and panel parameters and blocks saving until they are complete", async () => {
    const { onSave } = mount();
    fireEvent.click(screen.getByRole("button", { name: /Row 2, tile 2: Close/ }));
    await screen.findByRole("button", { name: "Press keys" });
    fireEvent.click(screen.getByRole("button", { name: "Press keys" }));
    // A template empties a label that only echoed the old action.
    expect(screen.getByRole("textbox", { name: "Label" })).toHaveValue("");
    expect(screen.getByText("Press the keys this tile should press.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save panel" })).toBeDisabled();
    // Tab alone leaves the recorder; a modifier alone records nothing.
    const recorder = screen.getByRole("textbox", { name: "Keys" });
    expect(fireEvent.keyDown(recorder, { key: "Tab" })).toBe(true);
    fireEvent.keyDown(recorder, { key: "Control", ctrlKey: true });
    expect(recorder).toHaveValue("");
    fireEvent.keyDown(recorder, { key: "s", ctrlKey: true });
    expect(screen.getByRole("textbox", { name: "Keys" })).toHaveValue("Ctrl + S");
    expect(screen.getByRole("textbox", { name: "Label" })).toHaveValue("Ctrl+S");
    expect(screen.getByRole("button", { name: "Save panel" })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: "Type text" }));
    expect(screen.getByRole("button", { name: "Save panel" })).toBeDisabled();
    fireEvent.change(screen.getByRole("textbox", { name: "Text to type" }), { target: { value: "Good morning everyone" } });
    expect(screen.getByRole("textbox", { name: "Label" })).toHaveValue("Good morning");

    fireEvent.click(screen.getByRole("button", { name: "Open one of my panels" }));
    expect(screen.getByText("Choose the panel this tile opens.")).toBeInTheDocument();
    const target = screen.getByRole("combobox", { name: "Panel to open" });
    expect(within(target).queryByRole("option", { name: "Greetings" })).not.toBeInTheDocument();
    fireEvent.change(target, { target: { value: "board" } });
    expect(screen.getByRole("textbox", { name: "Label" })).toHaveValue("Board");
    fireEvent.click(screen.getByRole("button", { name: "Save panel" }));
    await waitFor(() => expect(onSave).toHaveBeenCalled());
    expect(onSave.mock.calls[0][0].rows[1][1].action).toEqual({ type: "openCustomPanel", id: "board" });
  });

  it("adds, moves and removes tiles and rows within the limits", async () => {
    mount();
    fireEvent.click(screen.getByRole("button", { name: "Add tile to row 1" }));
    expect(screen.getByRole("heading", { name: "Row 1, tile 3" })).toBeInTheDocument();
    expect(within(screen.getByRole("group", { name: "Row 1" })).getAllByRole("button")).toHaveLength(3);
    fireEvent.click(screen.getByRole("button", { name: "Move tile left" }));
    expect(screen.getByRole("heading", { name: "Row 1, tile 2" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Add row" }));
    expect(screen.getByRole("group", { name: "Row 3" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Move row 3 up" }));
    expect(screen.getByRole("heading", { name: "Row 2, tile 1" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Remove tile" }));
    // The row it was alone on goes too.
    expect(screen.queryByRole("group", { name: "Row 3" })).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "No tile chosen" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Remove row 1" }));
    fireEvent.click(screen.getByRole("button", { name: "Remove row 1" }));
    expect(screen.getByText("Add at least one row.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save panel" })).toBeDisabled();
  });

  it("keeps focus and the user's labels when tiles and rows move", async () => {
    mount();
    fireEvent.click(screen.getByRole("button", { name: /Row 1, tile 1: Keyboard/ }));
    await screen.findByRole("button", { name: "Copy" });
    fireEvent.change(screen.getByRole("textbox", { name: "Label" }), { target: { value: "Letters" } });
    const right = screen.getByRole("button", { name: "Move tile right" });
    right.focus();
    fireEvent.click(right);
    expect(screen.getByRole("heading", { name: "Row 1, tile 2" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Move tile right" })).toHaveFocus();
    // The written label stays through a change of action after the move.
    fireEvent.click(screen.getByRole("button", { name: "Copy" }));
    expect(screen.getByRole("textbox", { name: "Label" })).toHaveValue("Letters");
    // Moving the other row up carries the chosen tile down with its row.
    fireEvent.click(screen.getByRole("button", { name: "Move row 2 up" }));
    expect(screen.getByRole("heading", { name: "Row 2, tile 2" })).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Label" })).toHaveValue("Letters");
  });

  it("asks before discarding changes or deleting", async () => {
    const { onClose, onDelete } = mount();
    fireEvent.click(screen.getByRole("button", { name: "All panels" }));
    expect(onClose).toHaveBeenCalledTimes(1);
    fireEvent.change(screen.getByRole("textbox", { name: "Panel name" }), { target: { value: "Changed" } });
    fireEvent.click(screen.getByRole("button", { name: "All panels" }));
    const discard = screen.getByRole("alertdialog", { name: "Discard unsaved changes?" });
    expect(within(discard).getByRole("button", { name: "Keep editing" })).toHaveFocus();
    fireEvent.click(within(discard).getByRole("button", { name: "Keep editing" }));
    expect(onClose).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.getByRole("button", { name: "All panels" })).toHaveFocus());
    fireEvent.click(screen.getByRole("button", { name: "Delete panel" }));
    const confirm = screen.getByRole("alertdialog", { name: "Delete Greetings?" });
    expect(within(confirm).getByRole("button", { name: "Keep editing" })).toHaveFocus();
    fireEvent.click(within(confirm).getByRole("button", { name: "Delete panel" }));
    await waitFor(() => expect(onDelete).toHaveBeenCalled());
  });
});
