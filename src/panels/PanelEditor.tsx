import { useEffect, useId, useMemo, useRef, useState } from "react";
import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, ChevronLeft, Plus, RotateCcw, Save, Trash2 } from "lucide-react";
import { Button, Input, Select } from "../ui/controls";
import { api } from "../api";
import { PanelPicture, toolbarAction } from "./PanelPicture";
import { actionProblem, chordFromEvent, defaultLabel, describeAction, friendlyKey, maxLabelChars, maxRows, maxSpan, maxTextChars, maxTilesPerRow, panelProblems, sameAction, searchActions } from "./actions";
import type { PanelTheme } from "../scanning/preferences";
import type { CustomPanel, PanelAction, PanelActionEntry, PanelCell } from "../types";

export const maxPanelNameChars = 50;
export const maxPanelDescriptionChars = 200;

type Selection = { row: number; cell: number };

const blankTile = (): PanelCell => ({ label: "Back", action: { type: "back" }, span: 1 });

/** Edits one panel: its name, its rows of tiles, and what each tile does.
 *  Nothing is saved until Save panel; leaving with changes asks first. */
/** A built-in menu being rearranged: its name stays, and Reset replaces Delete. */
export type BuiltinEditing = { kind: string; rearranged: boolean };

export function PanelEditor({ panel, panels, theme, busy, error, builtin, onClose, onSave, onDelete, onDirtyChange, onClearError }: {
  panel: CustomPanel; panels: CustomPanel[]; theme?: PanelTheme; busy: boolean; error: string | null; builtin?: BuiltinEditing;
  onClose: () => void; onSave: (panel: CustomPanel) => Promise<void>; onDelete: () => Promise<void>; onDirtyChange: (dirty: boolean) => void; onClearError?: () => void;
}) {
  const [draft, setDraft] = useState(panel);
  const [selected, setSelected] = useState<Selection | null>(null);
  // Counts the times a tile was chosen afresh, so the inspector's search is
  // cleared then and not when the same tile moves.
  const [pick, setPick] = useState(0);
  const choose = (selection: Selection) => { setSelected(selection); setPick((count) => count + 1); };
  const [confirmation, setConfirmation] = useState<"discard" | "delete" | null>(null);
  const [catalogue, setCatalogue] = useState<PanelActionEntry[]>([]);
  const confirmRef = useRef<HTMLButtonElement>(null);
  const backRef = useRef<HTMLButtonElement>(null);
  const deleteRef = useRef<HTMLButtonElement>(null);
  const addRowRef = useRef<HTMLButtonElement>(null);
  const ids = useId();
  const dirty = JSON.stringify(draft) !== JSON.stringify(panel);
  const problems = panelProblems(draft, panels, builtin?.kind);
  const canSave = dirty && Object.keys(problems).length === 0;

  useEffect(() => { onDirtyChange(dirty); return () => onDirtyChange(false); }, [dirty, onDirtyChange]);
  useEffect(() => { void api.panelActions().then(setCatalogue).catch(() => setCatalogue([])); }, []);
  useEffect(() => { if (confirmation) confirmRef.current?.focus(); }, [confirmation]);

  const update = (next: CustomPanel) => setDraft(next);
  const updateRows = (rows: PanelCell[][]) => update({ ...draft, rows });
  const current = selected ? draft.rows[selected.row]?.[selected.cell] ?? null : null;
  const setCell = (selection: Selection, cell: PanelCell) => updateRows(draft.rows.map((row, r) => r === selection.row ? row.map((candidate, c) => c === selection.cell ? cell : candidate) : row));
  /** Focus goes to a row's group once the tile that had it is gone. */
  const focusAfterRemoval = () => requestAnimationFrame(() => addRowRef.current?.focus());

  const addTile = (rowIndex: number) => {
    if (draft.rows[rowIndex].length >= maxTilesPerRow) return;
    updateRows(draft.rows.map((row, r) => r === rowIndex ? [...row, blankTile()] : row));
    choose({ row: rowIndex, cell: draft.rows[rowIndex].length });
  };
  const addRow = () => {
    if (draft.rows.length >= maxRows) return;
    updateRows([...draft.rows, [blankTile()]]);
    choose({ row: draft.rows.length, cell: 0 });
  };
  const removeTile = (selection: Selection) => {
    const rows = draft.rows.map((row, r) => r === selection.row ? row.filter((_, c) => c !== selection.cell) : row).filter((row) => row.length > 0);
    updateRows(rows);
    setSelected(null);
    focusAfterRemoval();
  };
  const removeRow = (rowIndex: number) => {
    updateRows(draft.rows.filter((_, r) => r !== rowIndex));
    setSelected(null);
    focusAfterRemoval();
  };
  const moveTile = (selection: Selection, direction: -1 | 1) => {
    const row = [...draft.rows[selection.row]];
    const target = selection.cell + direction;
    if (target < 0 || target >= row.length) return;
    [row[selection.cell], row[target]] = [row[target], row[selection.cell]];
    updateRows(draft.rows.map((candidate, r) => r === selection.row ? row : candidate));
    setSelected({ row: selection.row, cell: target });
  };
  const moveRow = (rowIndex: number, direction: -1 | 1) => {
    const target = rowIndex + direction;
    if (target < 0 || target >= draft.rows.length) return;
    const rows = [...draft.rows];
    [rows[rowIndex], rows[target]] = [rows[target], rows[rowIndex]];
    updateRows(rows);
    // The chosen tile travels with its row, whichever of the two it is in.
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
  const save = () => { if (canSave) void onSave({ ...draft, name: draft.name.trim(), description: draft.description.trim(), rows: draft.rows.map((row) => row.map((cell) => ({ ...cell, label: cell.label.trim() }))) }); };

  return <div className="view panel-editor">
    {confirmation && <div className="modal-backdrop"><section className="profile-dialog confirm-dialog" role="alertdialog" aria-modal="true" aria-labelledby={`${ids}-confirm`} tabIndex={-1} onKeyDown={(event) => { if (event.key === "Escape") { event.preventDefault(); keepEditing(); } }}>
      <header><div><h2 id={`${ids}-confirm`}>{confirmation === "delete" ? (builtin ? `Reset ${panel.name} to default?` : `Delete ${panel.name}?`) : "Discard unsaved changes?"}</h2><p>{confirmation === "delete" ? (builtin ? "The menu goes back to the way it came." : "Tiles on other panels that open it will say it no longer exists.") : "Your changes to this panel have not been saved."}</p></div></header>
      {error && <div className="dialog-error" role="alert">{error}</div>}
      <footer><span /><Button ref={confirmRef} className="secondary" onClick={keepEditing}>Keep editing</Button><Button className="primary danger" disabled={busy} onClick={() => { if (confirmation === "delete") void onDelete(); else onClose(); }}>{confirmation === "delete" ? (builtin ? "Reset to default" : "Delete panel") : "Discard changes"}</Button></footer>
    </section></div>}
    <div inert={confirmation !== null || undefined}>
      <header className="page-header"><div><Button ref={backRef} className="text-button panel-back" onClick={requestClose}><ChevronLeft size={18} aria-hidden="true" />All panels</Button><h1>{panel.name}</h1><p>{builtin ? (builtin.kind === "home" ? "Rearrange Home. Keep a Close tile, which ends scanning." : "Rearrange this built-in menu. Keep a Back or Close tile so it can be left.") : "Choose a tile to change what it says and does. Rows share their width between their tiles."}</p></div>
        <div className="panel-header-actions">{builtin
          ? <Button ref={deleteRef} className="secondary" disabled={busy || !builtin.rearranged} title={builtin.rearranged ? undefined : "Not changed yet"} onClick={requestDelete}><RotateCcw size={16} aria-hidden="true" />Reset to default</Button>
          : <Button ref={deleteRef} className="secondary danger" disabled={busy} onClick={requestDelete}><Trash2 size={16} aria-hidden="true" />Delete panel</Button>}<Button className="primary" disabled={busy || !canSave} onClick={save}><Save size={16} aria-hidden="true" />Save panel</Button></div></header>
      {error && !confirmation && <p className="dialog-error" role="alert">{error}</p>}
      <div className="panel-editor-layout">
        <div className="panel-editor-main">
          <form className="panel-fields" onSubmit={(event) => { event.preventDefault(); save(); }}>
            {!builtin && <>
              <label className="field"><span>Panel name</span><Input value={draft.name} maxLength={maxPanelNameChars} aria-invalid={problems.name ? true : undefined} aria-describedby={problems.name ? `${ids}-name-error` : undefined} onChange={(event) => update({ ...draft, name: event.target.value })} /></label>
              {problems.name && <p id={`${ids}-name-error`} className="field-error">{problems.name}</p>}
              <label className="field"><span>Description</span><Input value={draft.description} maxLength={maxPanelDescriptionChars} placeholder="What this panel is for" onChange={(event) => update({ ...draft, description: event.target.value })} /></label>
            </>}
          </form>
          <section className="panel-grid-editor" aria-label="Tiles">
            <h2>Tiles</h2>
            {problems.rows && <p className="field-error">{problems.rows}</p>}
            {draft.rows.map((row, rowIndex) => <div className="panel-editor-row" key={rowIndex} data-selected-row={selected?.row === rowIndex || undefined}>
              <div className="panel-editor-tiles" role="group" aria-label={`Row ${rowIndex + 1}`}>
                {row.map((cell, cellIndex) => {
                  const here = selected?.row === rowIndex && selected.cell === cellIndex;
                  const problem = problems[`tile-${rowIndex}-${cellIndex}`];
                  return <Button key={cellIndex} className="panel-editor-tile" style={{ flexGrow: cell.span, flexBasis: 0 }} data-selected={here || undefined} data-toolbar={toolbarAction(cell.action) || undefined} data-invalid={problem ? true : undefined} aria-pressed={here} aria-label={`Row ${rowIndex + 1}, tile ${cellIndex + 1}: ${cell.label.trim() || "no label"}, ${describeAction(cell.action, catalogue, panels)}${problem ? `, ${problem}` : ""}`} onClick={() => choose({ row: rowIndex, cell: cellIndex })}>{cell.label.trim() || <em>No label</em>}</Button>;
                })}
              </div>
              <div className="panel-editor-row-actions">
                <Button className="secondary" disabled={row.length >= maxTilesPerRow} aria-label={`Add tile to row ${rowIndex + 1}`} onClick={() => addTile(rowIndex)}><Plus size={16} aria-hidden="true" />Tile</Button>
                <Button className="icon-button" disabled={rowIndex === 0} aria-label={`Move row ${rowIndex + 1} up`} onClick={() => moveRow(rowIndex, -1)}><ArrowUp size={18} /></Button>
                <Button className="icon-button" disabled={rowIndex === draft.rows.length - 1} aria-label={`Move row ${rowIndex + 1} down`} onClick={() => moveRow(rowIndex, 1)}><ArrowDown size={18} /></Button>
                <Button className="icon-button danger-icon" aria-label={`Remove row ${rowIndex + 1}`} onClick={() => removeRow(rowIndex)}><Trash2 size={18} /></Button>
              </div>
              {problems[`row-${rowIndex}`] && <p className="field-error">{problems[`row-${rowIndex}`]}</p>}
            </div>)}
            <Button ref={addRowRef} className="secondary" disabled={draft.rows.length >= maxRows} onClick={addRow}><Plus size={16} aria-hidden="true" />Add row</Button>
          </section>
          <section className="panel-editor-picture" aria-label="Picture"><h2>As it will scan</h2><PanelPicture panel={draft} theme={theme} className="panel-picture-large" /></section>
        </div>
        <aside className="panel-inspector" aria-label="Selected tile">
          {selected && current
            ? <TileInspector pick={pick} selection={selected} cell={current} catalogue={catalogue} panels={panels} self={draft.id} problem={problems[`tile-${selected.row}-${selected.cell}`] ?? null} rowLength={draft.rows[selected.row].length}
              onChange={(cell) => setCell(selected, cell)} onAction={(action) => chooseAction(selected, current, action)} onMove={(direction) => moveTile(selected, direction)} onRemove={() => removeTile(selected)} />
            : <div className="panel-inspector-empty"><h2>No tile chosen</h2><p>Choose a tile on the left to change its label, width and action, or add a tile to a row.</p></div>}
        </aside>
      </div>
    </div>
  </div>;
}

function TileInspector({ pick, selection, cell, catalogue, panels, self, problem, rowLength, onChange, onAction, onMove, onRemove }: {
  pick: number; selection: Selection; cell: PanelCell; catalogue: PanelActionEntry[]; panels: CustomPanel[]; self: string; problem: string | null; rowLength: number;
  onChange: (cell: PanelCell) => void; onAction: (action: PanelAction) => void; onMove: (direction: -1 | 1) => void; onRemove: () => void;
}) {
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
    <h2>Row {selection.row + 1}, tile {selection.cell + 1}</h2>
    <label className="field"><span>Label</span><Input value={cell.label} maxLength={maxLabelChars} aria-invalid={labelProblem ? true : undefined} aria-describedby={labelProblem ? `${ids}-label-error` : undefined} onChange={(event) => onChange({ ...cell, label: event.target.value })} /></label>
    {labelProblem && <p id={`${ids}-label-error`} className="field-error">{labelProblem}</p>}
    <label className="field"><span>Width</span><Select value={cell.span} aria-label="Tile width" onChange={(event) => onChange({ ...cell, span: Number(event.target.value) })}>{Array.from({ length: maxSpan }, (_, index) => index + 1).map((span) => <option key={span} value={span}>{span === 1 ? "Normal" : `${span} tiles wide`}</option>)}</Select></label>
    <div className="tile-inspector-actions">
      <Button className="icon-button" disabled={selection.cell === 0} aria-label="Move tile left" onClick={() => onMove(-1)}><ArrowLeft size={18} /></Button>
      <Button className="icon-button" disabled={selection.cell === rowLength - 1} aria-label="Move tile right" onClick={() => onMove(1)}><ArrowRight size={18} /></Button>
      <Button className="secondary danger" onClick={onRemove}><Trash2 size={16} aria-hidden="true" />Remove tile</Button>
    </div>
    <section className="tile-action" aria-labelledby={`${ids}-action`}>
      <h3 id={`${ids}-action`}>Action</h3>
      <p className="tile-action-current" aria-live="polite">{describeAction(cell.action, catalogue, panels)}</p>
      {cell.action.type === "myPanels" && <p className="capability-note">Shown once you have made a panel.</p>}
      {cell.action.type === "key" && <label className="field"><span>Keys</span><Input className="key-recorder" placeholder="Select, then press the keys" value={cell.action.keys.map(friendlyKey).join(" + ")} readOnly aria-invalid={parameterProblem ? true : undefined} aria-describedby={parameterProblem ? `${ids}-parameter-error` : undefined} onKeyDown={recordKeys} /></label>}
      {cell.action.type === "text" && <label className="field"><span>Text to type</span><textarea className="field-control" rows={3} maxLength={maxTextChars} value={cell.action.text} aria-invalid={parameterProblem ? true : undefined} aria-describedby={parameterProblem ? `${ids}-parameter-error` : undefined} onChange={(event) => onAction({ type: "text", text: event.target.value })} /></label>}
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
