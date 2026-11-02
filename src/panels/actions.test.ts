import { describe, expect, it } from "vitest";
import { actionProblem, canonicalKey, chordFromEvent, defaultLabel, describeAction, panelProblems, searchActions } from "./actions";
import type { CustomPanel, PanelActionEntry } from "../types";

const catalogue: PanelActionEntry[] = [
  { id: "command.copy", name: "Copy", category: "editing", categoryLabel: "Editing", keywords: ["clipboard", "ctrl c"], action: { type: "command", command: "copy" }, needsPoint: false },
  { id: "command.switchNext", name: "Switch app forward", category: "windows", categoryLabel: "Apps and windows", keywords: ["alt tab", "application"], action: { type: "command", command: "switchNext" }, needsPoint: false },
  { id: "key", name: "Press keys", category: "typing", categoryLabel: "Typing", keywords: ["shortcut"], action: { type: "key", keys: [] }, parameter: { kind: "keys", min: 1, max: 4 }, needsPoint: false },
];

const panel = (id: string, name: string): CustomPanel => ({ id, version: 1, name, description: "", rows: [[{ label: "Back", action: { type: "back" }, span: 1 }]] });

describe("panel action helpers", () => {
  it("names keys the backend can press and rejects the rest", () => {
    expect(canonicalKey(" ")).toBe("Space");
    expect(canonicalKey("a")).toBe("A");
    expect(canonicalKey("Control")).toBe("Ctrl");
    expect(canonicalKey("é")).toBeNull();
    expect(canonicalKey("=")).toBeNull();
    expect(chordFromEvent({ key: "s", ctrlKey: true, altKey: false, shiftKey: false, metaKey: false })).toEqual(["Ctrl", "S"]);
    expect(chordFromEvent({ key: "Control", ctrlKey: true, altKey: false, shiftKey: false, metaKey: false })).toBeNull();
    expect(chordFromEvent({ key: "!", code: "Digit1", ctrlKey: false, altKey: false, shiftKey: true, metaKey: false })).toEqual(["Shift", "1"]);
    expect(chordFromEvent({ key: "Tab", ctrlKey: false, altKey: false, shiftKey: false, metaKey: false })).toEqual(["Tab"]);
  });

  it("reports what stops an action from being saved", () => {
    const panels = [panel("a", "A"), panel("b", "B")];
    expect(actionProblem({ type: "key", keys: [] }, panels, "a")).toMatch(/Press the keys/);
    expect(actionProblem({ type: "key", keys: ["Ctrl", "Shift"] }, panels, "a")).toMatch(/Add a key/);
    expect(actionProblem({ type: "key", keys: ["Ctrl", "S"] }, panels, "a")).toBeNull();
    expect(actionProblem({ type: "text", text: "  " }, panels, "a")).toMatch(/Type the text/);
    expect(actionProblem({ type: "text", text: "x".repeat(201) }, panels, "a")).toMatch(/200 characters/);
    expect(actionProblem({ type: "openCustomPanel", id: "" }, panels, "a")).toMatch(/Choose the panel/);
    expect(actionProblem({ type: "openCustomPanel", id: "a" }, panels, "a")).toMatch(/cannot open itself/);
    expect(actionProblem({ type: "openCustomPanel", id: "zzz" }, panels, "a")).toMatch(/no longer exists/);
    expect(actionProblem({ type: "openCustomPanel", id: "b" }, panels, "a")).toBeNull();
    expect(actionProblem({ type: "back" }, panels, "a")).toBeNull();
  });

  it("labels and describes actions like the backend", () => {
    const panels = [panel("b", "Board")];
    expect(defaultLabel({ type: "key", keys: ["Ctrl", "S"] }, catalogue, panels)).toBe("Ctrl+S");
    expect(defaultLabel({ type: "text", text: "Good morning everyone" }, catalogue, panels)).toBe("Good morning");
    expect(defaultLabel({ type: "command", command: "copy" }, catalogue, panels)).toBe("Copy");
    expect(defaultLabel({ type: "openCustomPanel", id: "b" }, catalogue, panels)).toBe("Board");
    expect(defaultLabel({ type: "openPanel", panel: "keyboard" }, catalogue, panels)).toBe("Keyboard");
    expect(defaultLabel({ type: "close" }, catalogue, panels)).toBe("Close");
    expect(defaultLabel({ type: "myPanels" }, catalogue, panels)).toBe("My panels");
    expect(defaultLabel({ type: "setting", setting: { keyboardLayout: "simpleQwerty" } }, catalogue, panels)).toBe("Simple QWERTY");
    const menus: PanelActionEntry[] = [{ id: "menu.editing", name: "Open Editing menu", category: "panels", categoryLabel: "Panels and menus", keywords: [], action: { type: "openPanel", panel: { menu: "editing" } }, needsPoint: false }];
    expect(defaultLabel({ type: "openPanel", panel: { menu: "editing" } }, menus, panels)).toBe("Editing");
    expect(describeAction({ type: "key", keys: ["Ctrl", "ArrowUp"] }, catalogue, panels)).toBe("Press Ctrl + Up Arrow");
    expect(describeAction({ type: "text", text: "Hi" }, catalogue, panels)).toBe('Type "Hi"');
    expect(describeAction({ type: "openCustomPanel", id: "b" }, catalogue, panels)).toBe("Open Board");
  });

  it("searches every word across name, category and keywords", () => {
    expect(searchActions(catalogue, "").length).toBe(3);
    expect(searchActions(catalogue, "CLIPBOARD").map((entry) => entry.id)).toEqual(["command.copy"]);
    expect(searchActions(catalogue, "alt tab").map((entry) => entry.id)).toEqual(["command.switchNext"]);
    expect(searchActions(catalogue, "apps forward").map((entry) => entry.id)).toEqual(["command.switchNext"]);
    expect(searchActions(catalogue, "nothing here")).toEqual([]);
  });

  it("finds every problem on a panel by field", () => {
    const other = panel("other", "Taken");
    const draft: CustomPanel = {
      id: "me", version: 1, name: " taken ", description: "",
      rows: [
        [{ label: "", action: { type: "back" }, span: 1 }, { label: "Keys", action: { type: "key", keys: [] }, span: 1 }],
        [],
      ],
    };
    const problems = panelProblems(draft, [other, draft]);
    expect(problems.name).toBe("Panel names must be unique.");
    expect(problems["tile-0-0"]).toBe("Give the tile a label.");
    expect(problems["tile-0-1"]).toMatch(/Press the keys/);
    expect(problems["row-1"]).toMatch(/Add a tile/);
    expect(Object.keys(panelProblems(panel("ok", "Ok"), [other]))).toEqual([]);
    // A built-in menu keeps its way out; Home keeps Close; its name need not be unique.
    const home: CustomPanel = { id: "builtin-home", version: 1, name: "Taken", description: "", rows: [[{ label: "Back", action: { type: "back" }, span: 1 }]] };
    expect(panelProblems(home, [other], "home").rows).toMatch(/Home keeps a Close/);
    expect(panelProblems(home, [other], "home").name).toBeUndefined();
    expect(panelProblems({ ...home, rows: [[{ label: "Copy", action: { type: "command", command: "copy" }, span: 1 }]] }, [], "editing").rows).toMatch(/Back or Close/);
    expect(Object.keys(panelProblems(home, [], "editing"))).toEqual([]);
  });
});
