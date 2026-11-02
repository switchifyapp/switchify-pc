import type { CustomPanel, PanelAction, PanelActionEntry, PanelCell } from "../types";

export const maxLabelChars = 40;
export const maxTextChars = 200;
export const maxChordKeys = 4;
export const maxRows = 12;
/** Rows a native panel takes below its keys, so it stays on screen. */
export const maxExtraRows = 4;
/** Columns a native panel takes beside its keys, and tiles in each. */
export const maxExtraColumns = 2;
export const maxColumnTiles = 6;

/** Problems with the columns beside a native panel's keys, keyed like
 *  `panelProblems`: `columns`, `column-N` and `ctile-N-M`. */
export function columnProblems(columns: PanelCell[][], panels: CustomPanel[], self: string): Record<string, string> {
  const problems: Record<string, string> = {};
  if (columns.length > maxExtraColumns) problems.columns = `A panel takes up to ${maxExtraColumns} extra columns.`;
  columns.forEach((column, columnIndex) => {
    if (column.length === 0) problems[`column-${columnIndex}`] = "Add a tile to this column or remove the column.";
    else if (column.length > maxColumnTiles) problems[`column-${columnIndex}`] = `A column holds up to ${maxColumnTiles} tiles.`;
    column.forEach((cell, cellIndex) => {
      const problem = cellProblem(cell, panels, self);
      if (problem) problems[`ctile-${columnIndex}-${cellIndex}`] = problem;
    });
  });
  return problems;
}
export const maxTilesPerRow = 8;
export const maxSpan = 4;

const modifierKeys = ["Ctrl", "Alt", "Shift", "Meta"];
const namedKeys = new Set(["Space", "Enter", "Escape", "Tab", "Backspace", "Delete", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown", ...modifierKeys, ...Array.from({ length: 12 }, (_, index) => `F${index + 1}`)]);
const friendlyKeys: Record<string, string> = { ArrowUp: "Up Arrow", ArrowDown: "Down Arrow", ArrowLeft: "Left Arrow", ArrowRight: "Right Arrow", Meta: "Command / Windows" };

/** The key name the backend knows for a DOM key, or null for one it cannot press. */
export function canonicalKey(key: string): string | null {
  const aliases: Record<string, string> = { " ": "Space", Esc: "Escape", Control: "Ctrl", OS: "Meta" };
  const canonical = aliases[key] ?? key;
  if (namedKeys.has(canonical)) return canonical;
  if (canonical.length === 1 && /^[a-z0-9]$/i.test(canonical)) return canonical.toUpperCase();
  return null;
}

export function friendlyKey(key: string) {
  return friendlyKeys[key] ?? key;
}

/** A chord from a keydown: the modifiers held, then the key. A modifier on
 *  its own is not a chord yet (the key is still to come), so it is null. The
 *  physical key code stands in when Shift turned a digit into a symbol. */
export function chordFromEvent(event: { key: string; code?: string; ctrlKey: boolean; altKey: boolean; shiftKey: boolean; metaKey: boolean }): string[] | null {
  const fromCode = event.code?.match(/^(?:Key([A-Z])|Digit([0-9]))$/);
  const pressed = canonicalKey(event.key) ?? (fromCode ? (fromCode[1] ?? fromCode[2]) : null);
  if (!pressed || modifierKeys.includes(pressed)) return null;
  const modifiers = [event.ctrlKey && "Ctrl", event.altKey && "Alt", event.shiftKey && "Shift", event.metaKey && "Meta"].filter((key): key is string => Boolean(key));
  return [...new Set([...modifiers, pressed])].slice(0, maxChordKeys);
}

/** Why an action cannot be saved yet, or null. Mirrors the backend's rules. */
export function actionProblem(action: PanelAction, panels: CustomPanel[], self: string): string | null {
  switch (action.type) {
    case "key":
      if (action.keys.length === 0) return "Press the keys this tile should press.";
      if (action.keys.every((key) => modifierKeys.includes(key))) return "Add a key to press with the modifiers.";
      return null;
    case "text":
      if (action.text.trim() === "") return "Type the text this tile should type.";
      if (action.text.length > maxTextChars) return `Text uses ${maxTextChars} characters or fewer.`;
      return null;
    case "openCustomPanel":
      if (action.id === "") return "Choose the panel this tile opens.";
      if (action.id === self) return "A panel cannot open itself.";
      if (!panels.some((panel) => panel.id === action.id)) return "That panel no longer exists.";
      return null;
    default:
      return null;
  }
}

/** The default tile label for an action, as the backend names it: short,
 *  since the library's names say more than fits on a tile. */
export function defaultLabel(action: PanelAction, catalogue: PanelActionEntry[], panels: CustomPanel[]): string {
  switch (action.type) {
    case "key": return action.keys.join("+");
    case "text": return action.text.replace(/\s/g, " ").slice(0, 12);
    case "openCustomPanel": return panels.find((panel) => panel.id === action.id)?.name ?? "Panel";
    case "openPanel": {
      if (typeof action.panel === "string") return { keyboard: "Keyboard", mouse: "Mouse", point: "Point", switchify: "Switchify" }[action.panel];
      const entry = catalogue.find((candidate) => sameAction(candidate.action, action));
      return entry ? entry.name.replace(/^Open /, "").replace(/ menu$/, "") : "Menu";
    }
    case "close": return "Close";
    case "myPanels": return "My panels";
    case "setting": {
      if (typeof action.setting === "object") return { qwerty: "QWERTY", simpleQwerty: "Simple QWERTY", commonLetters: "Common letters" }[action.setting.keyboardLayout];
      const entry = catalogue.find((candidate) => sameAction(candidate.action, action));
      return entry?.name ?? "Setting";
    }
    default: {
      const entry = catalogue.find((candidate) => sameAction(candidate.action, action));
      return entry?.name ?? action.type;
    }
  }
}

/** What a tile does, in words, for the inspector and screen readers. */
export function describeAction(action: PanelAction, catalogue: PanelActionEntry[], panels: CustomPanel[]): string {
  switch (action.type) {
    case "key": return action.keys.length ? `Press ${action.keys.map(friendlyKey).join(" + ")}` : "Press keys (none chosen yet)";
    case "text": return action.text ? `Type "${action.text}"` : "Type text (none yet)";
    case "openCustomPanel": {
      const panel = panels.find((candidate) => candidate.id === action.id);
      return panel ? `Open ${panel.name}` : "Open one of my panels (none chosen yet)";
    }
    default: return defaultLabel(action, catalogue, panels);
  }
}

export function sameAction(left: PanelAction, right: PanelAction) {
  return JSON.stringify(left) === JSON.stringify(right);
}

/** Every word of the query must match the name, category, keywords or id. */
export function searchActions(catalogue: PanelActionEntry[], query: string) {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return catalogue;
  return catalogue.filter((entry) => {
    const haystack = `${entry.name} ${entry.categoryLabel} ${entry.keywords.join(" ")} ${entry.id}`.toLowerCase();
    return words.every((word) => haystack.includes(word));
  });
}

/** Problems with the whole panel, keyed by field: `name`, `rows`, or `tile-r-c`.
 *  A built-in menu (`builtinKind` set) keeps its name and must keep a way out.
 *  Extra rows below a native panel's keys (`extras`) have no name, may be
 *  none at all, and are few. */
export function panelProblems(panel: CustomPanel, panels: CustomPanel[], builtinKind?: string, extras = false): Record<string, string> {
  const problems: Record<string, string> = {};
  const name = panel.name.trim();
  if (extras) { /* The panel's own name stays. */ }
  else if (name === "") problems.name = "Give the panel a name.";
  else if (name.length > 50) problems.name = "Panel names use 50 characters or fewer.";
  else if (!builtinKind && panels.some((other) => other.id !== panel.id && other.name.trim().toLowerCase() === name.toLowerCase())) problems.name = "Panel names must be unique.";
  if (builtinKind) {
    const actions = panel.rows.flat().map((cell) => cell.action.type);
    if (!actions.some((type) => type === "back" || type === "close")) problems.rows = "Keep a Back or Close tile so the menu can be left.";
    else if (builtinKind === "home" && !actions.includes("close")) problems.rows = "Home keeps a Close tile, which ends scanning.";
  }
  if (extras) { if (panel.rows.length > maxExtraRows) problems.rows = `A panel takes up to ${maxExtraRows} extra rows.`; }
  else if (panel.rows.length === 0) problems.rows = "Add at least one row.";
  else if (panel.rows.length > maxRows) problems.rows = `A panel has up to ${maxRows} rows.`;
  panel.rows.forEach((row, rowIndex) => {
    if (row.length === 0) problems[`row-${rowIndex}`] = "Add a tile to this row or remove the row.";
    else if (row.length > maxTilesPerRow) problems[`row-${rowIndex}`] = `A row holds up to ${maxTilesPerRow} tiles.`;
    row.forEach((cell, cellIndex) => {
      const problem = cellProblem(cell, panels, panel.id);
      if (problem) problems[`tile-${rowIndex}-${cellIndex}`] = problem;
    });
  });
  return problems;
}

export function cellProblem(cell: PanelCell, panels: CustomPanel[], self: string): string | null {
  const label = cell.label.trim();
  if (label === "") return "Give the tile a label.";
  if (label.length > maxLabelChars) return `Labels use ${maxLabelChars} characters or fewer.`;
  return actionProblem(cell.action, panels, self);
}
