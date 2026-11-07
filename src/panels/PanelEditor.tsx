import { ConfirmDialog } from "../ui/Dialog";
import { FeedbackMessage, PageHeader, FormField } from "../ui/patterns";
import { useEffect, useId, useMemo, useRef, useState } from "react";
import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, ChevronLeft, Play, Plus, RotateCcw, Save, Trash2 } from "lucide-react";
import { Textarea, IconButton, Button, Input, Select } from "../ui/controls";
import { api } from "../api";
import { PanelPicture, toolbarAction, type TilePlace } from "./PanelPicture";
import { actionProblem, chordFromEvent, columnProblems, defaultLabel, describeAction, friendlyKey, maxColumnTiles, maxExtraColumns, maxExtraRows, maxLabelChars, maxRows, maxSpan, maxTextChars, maxTilesPerRow, panelProblems, sameAction, searchActions } from "./actions";
import type { PanelTheme } from "../scanning/preferences";
import type { CustomPanel, PanelAction, PanelActionEntry, PanelCell, PictureCell } from "../types";

export const maxPanelNameChars = 50;
export const maxPanelDescriptionChars = 200;

/** A tile in a row, or (`column` set) a tile in one of a native panel's
 *  columns, where `row` is the column's index. */
type Selection = TilePlace;

const blankTile = (): PanelCell => ({ label: "Back", action: { type: "back" }, span: 1 });

/** Edits one panel: its name, its rows of tiles, and what each tile does.
 *  Nothing is saved until Save; leaving with changes asks first. */
/** A built-in menu being rearranged: its name stays, and Reset replaces Delete. */
export type BuiltinEditing = { kind: string; rearranged: boolean };
/** The tiles around a native panel's keys being edited: the keys are shown
 *  but fixed, there may be no rows or columns, and Remove tiles replaces
 *  Delete. */
export type ExtrasEditing = { target: string; fixed: PictureCell[][]; columns: PanelCell[][]; hasTiles: boolean };

export function PanelEditor({ panel, panels, theme, busy, error, builtin, extras, onClose, onSave, onSaveExtras, onDelete, onTry, onDirtyChange, onClearError }: {
  panel: CustomPanel; panels: CustomPanel[]; theme?: PanelTheme; busy: boolean; error: string | null; builtin?: BuiltinEditing; extras?: ExtrasEditing;
  onClose: () => void; onSave: (panel: CustomPanel) => Promise<void>; onSaveExtras?: (rows: PanelCell[][], columns: PanelCell[][]) => Promise<void>; onDelete: () => Promise<void>; onTry?: () => Promise<void>; onDirtyChange: (dirty: boolean) => void; onClearError?: () => void;
}) {
  const [draft, setDraft] = useState(panel);
  const [columns, setColumns] = useState<PanelCell[][]>(extras?.columns ?? []);
  const [selected, setSelected] = useState<Selection | null>(null);
  // Counts the times a tile was chosen afresh, so the inspector's search is
  // cleared then and not when the same tile moves.
  const [pick, setPick] = useState(0);
  const choose = (selection: Selection) => { setSelected(selection); setPick((count) => count + 1); };
  const [confirmation, setConfirmation] = useState<"discard" | "delete" | null>(null);
  const [catalogue, setCatalogue] = useState<PanelActionEntry[]>([]);
  const backRef = useRef<HTMLButtonElement>(null);
  const deleteRef = useRef<HTMLButtonElement>(null);
  const addRowRef = useRef<HTMLButtonElement>(null);
  const addColumnRef = useRef<HTMLButtonElement>(null);
  const nameRef = useRef<HTMLInputElement>(null);
  const rowsRef = useRef<HTMLElement>(null);
  const ids = useId();
  const dirty = JSON.stringify(draft) !== JSON.stringify(panel) || JSON.stringify(columns) !== JSON.stringify(extras?.columns ?? []);
  const problems = { ...panelProblems(draft, panels, builtin?.kind, extras !== undefined), ...(extras ? columnProblems(columns, panels, draft.id) : {}) };
  const canSave = dirty && Object.keys(problems).length === 0;
  const rowLimit = extras ? maxExtraRows : maxRows;
  const saveLabel = extras || builtin ? "Save changes" : "Save panel";
  /** Brings an element into view and gives it focus. */
  const reveal = (element: HTMLElement | null) => { element?.scrollIntoView?.({ block: "center" }); element?.focus(); };
  const tileId = (selection: Selection) => `${ids}-${selection.column ? "ctile" : "tile"}-${selection.row}-${selection.cell}`;
  /** What stops the panel saving, each with a way to the place it is about,
   *  so a problem out of view is never a mystery. */
  const attention = Object.entries(problems).map(([key, message]) => {
    const tile = key.match(/^(c?)tile-(\d+)-(\d+)$/);
    if (tile) {
      const selection: Selection = { row: Number(tile[2]), cell: Number(tile[3]), ...(tile[1] ? { column: true as const } : {}) };
      return { label: `${selection.column ? "Column" : "Row"} ${selection.row + 1}, tile ${selection.cell + 1}: ${message}`, go: () => { choose(selection); requestAnimationFrame(() => reveal(document.getElementById(tileId(selection)))); } };
    }
    const line = key.match(/^(row|column)-(\d+)$/);
    if (line) {
      const place = `${line[1] === "row" ? "Row" : "Column"} ${Number(line[2]) + 1}`;
      return { label: `${place}: ${message}`, go: () => reveal(document.querySelector<HTMLElement>(`[aria-label="Add tile to ${place.toLowerCase()}"]`)) };
    }
    const section = key === "name" ? nameRef : rowsRef;
    return { label: `To save: ${message}`, go: () => reveal(section.current) };
  });

  useEffect(() => { onDirtyChange(dirty); return () => onDirtyChange(false); }, [dirty, onDirtyChange]);
  useEffect(() => { void api.panelActions().then(setCatalogue).catch(() => setCatalogue([])); }, []);

  const update = (next: CustomPanel) => setDraft(next);
  const updateRows = (rows: PanelCell[][]) => update({ ...draft, rows });
  const current = selected ? (selected.column ? columns : draft.rows)[selected.row]?.[selected.cell] ?? null : null;
  const replaceIn = (lines: PanelCell[][], selection: Selection, cell: PanelCell) => lines.map((line, r) => r === selection.row ? line.map((candidate, c) => c === selection.cell ? cell : candidate) : line);
  const setCell = (selection: Selection, cell: PanelCell) => selection.column ? setColumns(replaceIn(columns, selection, cell)) : updateRows(replaceIn(draft.rows, selection, cell));
  const problemOf = (selection: Selection) => problems[selection.column ? `ctile-${selection.row}-${selection.cell}` : `tile-${selection.row}-${selection.cell}`] ?? null;
  /** Focus goes to the section's Add button once the tile that had it is gone. */
  const focusAfterRemoval = (column = false) => requestAnimationFrame(() => (column ? addColumnRef : addRowRef).current?.focus());

  const addTile = (rowIndex: number) => {
    if (draft.rows[rowIndex].length >= maxTilesPerRow) return;
    updateRows(draft.rows.map((row, r) => r === rowIndex ? [...row, blankTile()] : row));
    choose({ row: rowIndex, cell: draft.rows[rowIndex].length });
  };
  const addRow = () => {
    if (draft.rows.length >= rowLimit) return;
    updateRows([...draft.rows, [blankTile()]]);
    choose({ row: draft.rows.length, cell: 0 });
  };
  const removeTile = (selection: Selection) => {
    const lines = (selection.column ? columns : draft.rows).map((line, r) => r === selection.row ? line.filter((_, c) => c !== selection.cell) : line).filter((line) => line.length > 0);
    if (selection.column) setColumns(lines); else updateRows(lines);
    setSelected(null);
    focusAfterRemoval(selection.column === true);
  };
  const addColumnTile = (columnIndex: number) => {
    if (columns[columnIndex].length >= maxColumnTiles) return;
    setColumns(columns.map((column, c) => c === columnIndex ? [...column, blankTile()] : column));
    choose({ row: columnIndex, cell: columns[columnIndex].length, column: true });
  };
  const addColumn = () => {
    if (columns.length >= maxExtraColumns) return;
    setColumns([...columns, [blankTile()]]);
    choose({ row: columns.length, cell: 0, column: true });
  };
  const removeColumn = (columnIndex: number) => {
    setColumns(columns.filter((_, c) => c !== columnIndex));
    setSelected(null);
    focusAfterRemoval(true);
  };
  const moveColumn = (columnIndex: number, direction: -1 | 1) => {
    const target = columnIndex + direction;
    if (target < 0 || target >= columns.length) return;
    const next = [...columns];
    [next[columnIndex], next[target]] = [next[target], next[columnIndex]];
    setColumns(next);
    setSelected(selected?.column && selected.row === columnIndex ? { ...selected, row: target } : selected?.column && selected.row === target ? { ...selected, row: columnIndex } : selected);
  };
  const removeRow = (rowIndex: number) => {
    updateRows(draft.rows.filter((_, r) => r !== rowIndex));
    setSelected(null);
    focusAfterRemoval();
  };
  /** Moves a tile to the end of another row or column, or a new one. A row
   *  or column left empty goes with it. */
  const moveTileTo = (selection: Selection, target: number | "new") => {
    const lines = selection.column ? columns : draft.rows;
    const cell = lines[selection.row]?.[selection.cell];
    if (!cell) return;
    let next = lines.map((line, r) => r === selection.row ? line.filter((_, c) => c !== selection.cell) : line);
    let index = target === "new" ? next.length : target;
    if (target === "new") next = [...next, []];
    next = next.map((line, r) => r === index ? [...line, cell] : line);
    if (next[selection.row].length === 0) {
      next = next.filter((_, r) => r !== selection.row);
      if (index > selection.row) index -= 1;
    }
    if (selection.column) setColumns(next); else updateRows(next);
    setSelected({ ...selection, row: index, cell: next[index].length - 1 });
  };
  const moveTile = (selection: Selection, direction: -1 | 1) => {
    const lines = selection.column ? columns : draft.rows;
    const line = [...lines[selection.row]];
    const target = selection.cell + direction;
    if (target < 0 || target >= line.length) return;
    [line[selection.cell], line[target]] = [line[target], line[selection.cell]];
    const next = lines.map((candidate, r) => r === selection.row ? line : candidate);
    if (selection.column) setColumns(next); else updateRows(next);
    setSelected({ ...selection, cell: target });
  };
  const moveRow = (rowIndex: number, direction: -1 | 1) => {
    const target = rowIndex + direction;
    if (target < 0 || target >= draft.rows.length) return;
    const rows = [...draft.rows];
    [rows[rowIndex], rows[target]] = [rows[target], rows[rowIndex]];
    updateRows(rows);
    // The chosen tile travels with its row, whichever of the two it is in.
    if (selected?.column) return;
    setSelected(selected?.row === rowIndex ? { ...selected, row: target } : selected?.row === target ? { ...selected, row: rowIndex } : selected);
  };
  /** Gives the tile a new action. A label the user wrote stays; one that
   *  only echoed the old action follows the new one. */
  const chooseAction = (selection: Selection, cell: PanelCell, action: PanelAction) => {
    const own = cell.label.trim() !== "" && cell.label !== defaultLabel(cell.action, catalogue, panels);
    const label = own ? cell.label : defaultLabel(action, catalogue, panels).slice(0, maxLabelChars);
    setCell(selection, { ...cell, action, label });
  };

  const requestClose = () => { if (!dirty) { onClose(); return; } onClearError?.(); setConfirmation("discard"); };
  const requestDelete = () => { onClearError?.(); setConfirmation("delete"); };
  const keepEditing = () => {
    const opener = confirmation === "delete" ? deleteRef.current : backRef.current;
    setConfirmation(null);
    requestAnimationFrame(() => opener?.focus());
  };
  const trimmed = (lines: PanelCell[][]) => lines.map((line) => line.map((cell) => ({ ...cell, label: cell.label.trim() })));
  const save = () => {
    if (!canSave) return;
    if (extras && onSaveExtras) void onSaveExtras(trimmed(draft.rows), trimmed(columns));
    else void onSave({ ...draft, name: draft.name.trim(), description: draft.description.trim(), rows: trimmed(draft.rows) });
  };

  return <div className="view panel-editor">
    {confirmation && <ConfirmDialog
      title={confirmation === "delete" ? (extras ? `Remove the tiles added to ${panel.name}?` : builtin ? `Reset ${panel.name} to default?` : `Delete ${panel.name}?`) : "Discard unsaved changes?"}
      description={confirmation === "delete" ? (extras ? "The panel goes back to its keys alone." : builtin ? "The menu goes back to the way it came." : "Tiles on other panels that open it will say it no longer exists.") : "Your changes to this panel have not been saved."}
      action={confirmation === "delete" ? (extras ? "Remove tiles" : builtin ? "Reset to default" : "Delete panel") : "Discard changes"}
      busy={busy} error={error} onCancel={keepEditing} onConfirm={() => { if (confirmation === "delete") void onDelete(); else onClose(); }}
    />}
    <div inert={confirmation !== null || undefined}>
      <PageHeader title={panel.name} before={<Button ref={backRef} className="text-button panel-back" onClick={requestClose}><ChevronLeft size={18} aria-hidden="true" />All panels</Button>} description={<>{extras ? "Add rows of your own below the keys, or columns beside them. The keys stay as they are; your tiles scan with them." : builtin ? (builtin.kind === "home" ? "Rearrange Home. Keep a Close tile, which ends scanning." : "Rearrange this built-in menu. Keep a Back or Close tile so it can be left.") : "Choose a tile to change what it says and does. Rows share their width between their tiles."}</>} action={
        <div className="panel-header-actions">{onTry && <Button className="secondary" disabled={busy || dirty} title={dirty ? "Save first, then try" : "Open this panel in the scanner"} onClick={() => { onClearError?.(); void onTry(); }}><Play size={16} aria-hidden="true" />Try it</Button>}{extras
          ? <Button ref={deleteRef} className="secondary danger" disabled={busy || !extras.hasTiles} title={extras.hasTiles ? undefined : "No tiles added yet"} onClick={requestDelete}><Trash2 size={16} aria-hidden="true" />Remove tiles</Button>
          : builtin
          ? <Button ref={deleteRef} className="secondary" disabled={busy || !builtin.rearranged} title={builtin.rearranged ? undefined : "Not changed yet"} onClick={requestDelete}><RotateCcw size={16} aria-hidden="true" />Reset to default</Button>
          : <Button ref={deleteRef} className="secondary danger" disabled={busy} onClick={requestDelete}><Trash2 size={16} aria-hidden="true" />Delete panel</Button>}<Button className="primary" disabled={busy || !canSave} onClick={save}><Save size={16} aria-hidden="true" />{saveLabel}</Button></div>} />
      {/* Always present, so screen readers hear a problem appear. */}
      <p className="save-hint" role="status" data-empty={attention.length === 0 || undefined}>{attention.length > 0 && <Button className="text-button" onClick={attention[0].go}>{attention.length === 1 ? attention[0].label : `${attention.length} things need attention. Go to the first`}</Button>}</p>
      {error && !confirmation && <FeedbackMessage>{error}</FeedbackMessage>}
      <div className="panel-editor-layout">
        <div className="panel-editor-main">
          <form className="panel-fields" onSubmit={(event) => { event.preventDefault(); save(); }}>
            {!builtin && !extras && <>
              <FormField label="Panel name" error={problems.name}>{(field) => <Input {...field} ref={nameRef} value={draft.name} maxLength={maxPanelNameChars} onChange={(event) => update({ ...draft, name: event.target.value })} />}</FormField>
              <FormField label="Description">{(field) => <Input {...field} value={draft.description} maxLength={maxPanelDescriptionChars} placeholder="What this panel is for" onChange={(event) => update({ ...draft, description: event.target.value })} />}</FormField>
            </>}
          </form>
          <section ref={rowsRef} tabIndex={-1} className="panel-grid-editor" aria-label="Tiles">
            <h2>As it will scan</h2>
            <p className="capability-note">{extras ? "Choose one of your tiles to change it. Row tools sit beside the picture; labelled column tools sit below it." : "Choose a tile to change it. Each row has its tools beside it."} Wide pictures scroll sideways while you edit.</p>
            {problems.rows && <p className="field-error">{problems.rows}</p>}
            {problems.columns && <p className="field-error">{problems.columns}</p>}
            {[...draft.rows.map((_, index) => [`row-${index}`, `Row ${index + 1}`]), ...columns.map((_, index) => [`column-${index}`, `Column ${index + 1}`])].map(([key, place]) => problems[key] && <p key={key} className="field-error">{place}: {problems[key]}</p>)}
            <PanelPicture panel={draft} fixed={extras?.fixed} columns={extras ? columns : undefined} theme={theme} className="panel-picture-large panel-picture-editing" editing={{
              tile: (cell, place) => {
                const here = selected?.column === place.column && selected?.row === place.row && selected?.cell === place.cell;
                const problem = problemOf(place);
                const where = `${place.column ? "Column" : "Row"} ${place.row + 1}, tile ${place.cell + 1}`;
                return <Button id={tileId(place)} className="panel-editor-tile" data-selected={here || undefined} data-toolbar={toolbarAction(cell.action) || undefined} data-invalid={problem ? true : undefined} aria-pressed={here} aria-label={`${where}: ${cell.label.trim() || "no label"}, ${describeAction(cell.action, catalogue, panels)}${problem ? `, ${problem}` : ""}`} onClick={() => choose(place)}>{cell.label.trim() || <em>No label</em>}</Button>;
              },
              rowTools: (rowIndex) => <>
                <IconButton disabled={draft.rows[rowIndex].length >= maxTilesPerRow} label={`Add tile to row ${rowIndex + 1}`} onClick={() => addTile(rowIndex)}><Plus size={16} /></IconButton>
                <IconButton disabled={rowIndex === 0} label={`Move row ${rowIndex + 1} up`} onClick={() => moveRow(rowIndex, -1)}><ArrowUp size={16} /></IconButton>
                <IconButton disabled={rowIndex === draft.rows.length - 1} label={`Move row ${rowIndex + 1} down`} onClick={() => moveRow(rowIndex, 1)}><ArrowDown size={16} /></IconButton>
                <IconButton className="danger-icon" label={`Remove row ${rowIndex + 1}`} onClick={() => removeRow(rowIndex)}><Trash2 size={16} /></IconButton>
              </>,
              columnTools: (columnIndex) => <>
                <IconButton disabled={columns[columnIndex].length >= maxColumnTiles} label={`Add tile to column ${columnIndex + 1}`} onClick={() => addColumnTile(columnIndex)}><Plus size={16} /></IconButton>
                <IconButton disabled={columnIndex === 0} label={`Move column ${columnIndex + 1} left`} onClick={() => moveColumn(columnIndex, -1)}><ArrowLeft size={16} /></IconButton>
                <IconButton disabled={columnIndex === columns.length - 1} label={`Move column ${columnIndex + 1} right`} onClick={() => moveColumn(columnIndex, 1)}><ArrowRight size={16} /></IconButton>
                <IconButton className="danger-icon" label={`Remove column ${columnIndex + 1}`} onClick={() => removeColumn(columnIndex)}><Trash2 size={16} /></IconButton>
              </>,
            }} />
            <div className="panel-canvas-actions">
              <Button ref={addRowRef} className="secondary" disabled={draft.rows.length >= rowLimit} onClick={addRow}><Plus size={16} aria-hidden="true" />Add row</Button>
              {extras && <Button ref={addColumnRef} className="secondary" disabled={columns.length >= maxExtraColumns} onClick={addColumn}><Plus size={16} aria-hidden="true" />Add column</Button>}
            </div>
            {extras && draft.rows.length === 0 && columns.length === 0 && <p className="capability-note">No tiles of your own yet. Add a row below the keys or a column beside them, then give its tiles labels and actions.</p>}
          </section>
        </div>
        <aside className="panel-inspector" aria-label="Selected tile">
          {selected && current
            ? <TileInspector pick={pick} selection={selected} cell={current} catalogue={catalogue} panels={panels} self={draft.id} problem={problemOf(selected)} lines={selected.column ? columns : draft.rows} lineLimit={selected.column ? maxExtraColumns : rowLimit} tileLimit={selected.column ? maxColumnTiles : maxTilesPerRow}
              onChange={(cell) => setCell(selected, cell)} onAction={(action) => chooseAction(selected, current, action)} onMove={(direction) => moveTile(selected, direction)} onMoveTo={(target) => moveTileTo(selected, target)} onRemove={() => removeTile(selected)} />
            : <div className="panel-inspector-empty"><h2>No tile chosen</h2><p>Choose a tile on the left to change its label, width and action, or add a tile to a row.</p></div>}
        </aside>
      </div>
    </div>
  </div>;
}

function TileInspector({ pick, selection, cell, catalogue, panels, self, problem, lines, lineLimit, tileLimit, onChange, onAction, onMove, onMoveTo, onRemove }: {
  pick: number; selection: Selection; cell: PanelCell; catalogue: PanelActionEntry[]; panels: CustomPanel[]; self: string; problem: string | null;
  /** The rows, or the columns when the tile is in one, and how many of each may exist. */
  lines: PanelCell[][]; lineLimit: number; tileLimit: number;
  onChange: (cell: PanelCell) => void; onAction: (action: PanelAction) => void; onMove: (direction: -1 | 1) => void; onMoveTo: (target: number | "new") => void; onRemove: () => void;
}) {
  const rowLength = lines[selection.row].length;
  const lineNoun = selection.column ? "column" : "row";
  const [query, setQuery] = useState("");
  const ids = useId();
  // A newly chosen tile starts with an empty search; the inspector itself stays mounted so focus does.
  useEffect(() => setQuery(""), [pick]);
  const others = useMemo(() => panels.filter((candidate) => candidate.id !== self), [panels, self]);
  const results = useMemo(() => searchActions(catalogue, query), [catalogue, query]);
  const grouped = useMemo(() => {
    const groups = new Map<string, PanelActionEntry[]>();
    for (const entry of results) groups.set(entry.categoryLabel, [...(groups.get(entry.categoryLabel) ?? []), entry]);
    return [...groups.entries()];
  }, [results]);
  const chosenEntry = catalogue.find((entry) => entry.parameter ? entry.action.type === cell.action.type : sameAction(entry.action, cell.action));
  const parameterProblem = actionProblem(cell.action, panels, self);
  const labelProblem = problem && !parameterProblem ? problem : null;
  const recordKeys = (event: React.KeyboardEvent<HTMLInputElement>) => {
    // Tab on its own still moves focus, so the field never traps a keyboard user.
    if (event.key === "Tab" && !event.ctrlKey && !event.altKey && !event.metaKey) return;
    event.preventDefault();
    const keys = chordFromEvent(event);
    if (keys) onAction({ type: "key", keys });
  };
  return <div className="tile-inspector">
    <h2>{selection.column ? "Column" : "Row"} {selection.row + 1}, tile {selection.cell + 1}</h2>
    <label className="field"><span>Label</span><Input value={cell.label} maxLength={maxLabelChars} aria-invalid={labelProblem ? true : undefined} aria-describedby={labelProblem ? `${ids}-label-error` : undefined} onChange={(event) => onChange({ ...cell, label: event.target.value })} /></label>
    {labelProblem && <p id={`${ids}-label-error`} className="field-error">{labelProblem}</p>}
    <label className="field"><span>Width</span><Select value={cell.span} aria-label="Tile width" onChange={(event) => onChange({ ...cell, span: Number(event.target.value) })}>{Array.from({ length: maxSpan }, (_, index) => index + 1).map((span) => <option key={span} value={span}>{span === 1 ? "Normal" : `${span} tiles wide`}</option>)}</Select></label>
    <label className="field"><span>{selection.column ? "Column" : "Row"}</span><Select value={String(selection.row)} aria-label={`Move tile to ${lineNoun}`} onChange={(event) => { const value = event.target.value; if (value === "new") onMoveTo("new"); else if (Number(value) !== selection.row) onMoveTo(Number(value)); }}>
      {lines.map((line, index) => <option key={index} value={index} disabled={index !== selection.row && line.length >= tileLimit}>{selection.column ? "Column" : "Row"} {index + 1}{index === selection.row ? " (here)" : line.length >= tileLimit ? " (full)" : ""}</option>)}
      <option value="new" disabled={lines.length >= lineLimit}>New {lineNoun}{lines.length >= lineLimit ? " (no room)" : ""}</option>
    </Select></label>
    <div className="tile-inspector-actions">
      <IconButton disabled={selection.cell === 0} label={selection.column ? "Move tile up" : "Move tile left"} onClick={() => onMove(-1)}>{selection.column ? <ArrowUp size={18} /> : <ArrowLeft size={18} />}</IconButton>
      <IconButton disabled={selection.cell === rowLength - 1} label={selection.column ? "Move tile down" : "Move tile right"} onClick={() => onMove(1)}>{selection.column ? <ArrowDown size={18} /> : <ArrowRight size={18} />}</IconButton>
      <Button className="secondary danger" onClick={onRemove}><Trash2 size={16} aria-hidden="true" />Remove tile</Button>
    </div>
    <section className="tile-action" aria-labelledby={`${ids}-action`}>
      <h3 id={`${ids}-action`}>Action</h3>
      <p className="tile-action-current" aria-live="polite">{describeAction(cell.action, catalogue, panels)}</p>
      {cell.action.type === "myPanels" && <p className="capability-note">Shown once you have made a panel.</p>}
      {cell.action.type === "key" && <label className="field"><span>Keys</span><Input className="key-recorder" placeholder="Select, then press the keys" value={cell.action.keys.map(friendlyKey).join(" + ")} readOnly aria-invalid={parameterProblem ? true : undefined} aria-describedby={parameterProblem ? `${ids}-parameter-error` : undefined} onKeyDown={recordKeys} /></label>}
      {cell.action.type === "text" && <label className="field"><span>Text to type</span><Textarea rows={3} maxLength={maxTextChars} value={cell.action.text} aria-invalid={parameterProblem ? true : undefined} aria-describedby={parameterProblem ? `${ids}-parameter-error` : undefined} onChange={(event) => onAction({ type: "text", text: event.target.value })} /></label>}
      {cell.action.type === "openCustomPanel" && <label className="field"><span>Panel to open</span><Select value={cell.action.id} aria-invalid={parameterProblem ? true : undefined} aria-describedby={parameterProblem ? `${ids}-parameter-error` : undefined} onChange={(event) => onAction({ type: "openCustomPanel", id: event.target.value })}><option value="">Choose a panel</option>{others.map((candidate) => <option key={candidate.id} value={candidate.id}>{candidate.name}</option>)}</Select></label>}
      {parameterProblem && <p id={`${ids}-parameter-error`} className="field-error">{parameterProblem}</p>}
      <label className="field"><span>Change action</span><Input type="search" placeholder="Search actions, for example copy or volume" value={query} onChange={(event) => setQuery(event.target.value)} /></label>
      <div className="action-list" aria-label="Actions">
        {grouped.length === 0 && <p className="capability-note">{catalogue.length === 0 ? "The action library is loading." : "No actions match. Try another word."}</p>}
        {grouped.map(([category, entries]) => <div className="action-group" role="group" aria-label={category} key={category}><h4>{category}</h4>
          {entries.map((entry) => <Button key={entry.id} className="action-option" aria-pressed={chosenEntry?.id === entry.id} onClick={() => onAction(entry.parameter && entry.action.type === cell.action.type ? cell.action : entry.action)}>{entry.name}{entry.needsPoint && <small>at the pointer</small>}</Button>)}
        </div>)}
      </div>
    </section>
  </div>;
}
