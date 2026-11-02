import { useEffect, useId, useRef, useState } from "react";
import { ChevronLeft, LayoutGrid, Plus, Trash2 } from "lucide-react";
import { Button, Input } from "../ui/controls";
import { PanelPicture } from "./PanelPicture";
import type { PanelsController } from "./usePanels";
import type { PanelTheme } from "../scanning/preferences";
import type { CustomPanel } from "../types";

export const maxPanelNameChars = 50;
export const maxPanelDescriptionChars = 200;

/** A first panel to build on: the two panels people open most, then Back and Close. */
export function starterPanel(existing: CustomPanel[]): CustomPanel {
  const taken = new Set(existing.map((panel) => panel.name.trim().toLowerCase()));
  let name = "New panel";
  for (let n = 2; taken.has(name.toLowerCase()); n++) name = `New panel ${n}`;
  return {
    id: crypto.randomUUID(),
    version: 1,
    name,
    description: "",
    rows: [
      [{ label: "Keyboard", action: { type: "openPanel", panel: "keyboard" }, span: 1 }, { label: "Mouse", action: { type: "openPanel", panel: "mouse" }, span: 1 }],
      [{ label: "Back", action: { type: "back" }, span: 1 }, { label: "Close", action: { type: "close" }, span: 1 }],
    ],
  };
}

export function tileCount(panel: CustomPanel) {
  return panel.rows.reduce((count, row) => count + row.length, 0);
}

export function PanelsView({ controller, theme, onDirtyChange }: { controller: PanelsController; theme?: PanelTheme; onDirtyChange: (dirty: boolean) => void }) {
  const { panels, busy, error, clearError, save, remove, setEditing } = controller;
  const [open, setOpen] = useState<CustomPanel | null>(null);
  const openerRef = useRef<HTMLButtonElement | null>(null);
  // Leaving the page by any route lets sync refresh the list again.
  useEffect(() => () => setEditing(false), [setEditing]);
  const openPanel = (panel: CustomPanel, opener: HTMLButtonElement | null) => { openerRef.current = opener; clearError(); setEditing(true); setOpen(panel); };
  const closePanel = () => {
    setEditing(false);
    setOpen(null);
    requestAnimationFrame(() => (openerRef.current?.isConnected ? openerRef.current : document.querySelector<HTMLButtonElement>(".page-header button"))?.focus());
  };
  const create = async () => {
    const panel = starterPanel(panels ?? []);
    const saved = await save(panel).then((list) => list.find((candidate) => candidate.id === panel.id) ?? panel, () => null);
    if (saved === null) return;
    openPanel(saved, null);
  };
  if (open) return <PanelDetail panel={open} panels={panels ?? []} theme={theme} busy={busy} error={error} onDirtyChange={onDirtyChange} onClose={closePanel} onSave={(panel) => save(panel).then(closePanel, () => undefined)} onDelete={() => remove(open.id).then(closePanel, () => undefined)} />;
  return <div className="view">
    <header className="page-header"><div><h1>Panels</h1><p>Scanning panels of your own. Home lists them under My panels.</p></div><Button className="primary" disabled={busy || panels === null} onClick={() => void create()}><Plus size={16} />New panel</Button></header>
    {error && <p className="field-error" role="alert">{error}</p>}
    {panels === null ? <p className="capability-note">Loading your panels...</p>
      : panels.length === 0 ? <section className="empty-state" aria-label="No panels yet"><LayoutGrid size={32} aria-hidden="true" /><h2>No panels yet</h2><p>A panel is a page of tiles that scans like Home. Each tile presses keys, types a phrase, runs an action or opens another panel. Make one and it appears in Home under My panels.</p></section>
      : <ul className="panel-grid" aria-label="Your panels">{panels.map((panel) => <li key={panel.id}>
        <Button className="panel-card" onClick={(event) => openPanel(panel, event.currentTarget)} aria-label={`${panel.name}, ${tileCount(panel)} tiles`}>
          <PanelPicture panel={panel} theme={theme} />
          <span className="panel-card-text"><strong>{panel.name}</strong><small>{panel.description.trim() || `${tileCount(panel)} ${tileCount(panel) === 1 ? "tile" : "tiles"}`}</small></span>
        </Button>
      </li>)}</ul>}
  </div>;
}

function PanelDetail({ panel, panels, theme, busy, error, onDirtyChange, onClose, onSave, onDelete }: { panel: CustomPanel; panels: CustomPanel[]; theme?: PanelTheme; busy: boolean; error: string | null; onDirtyChange: (dirty: boolean) => void; onClose: () => void; onSave: (panel: CustomPanel) => Promise<void>; onDelete: () => Promise<void> }) {
  const [draft, setDraft] = useState(panel);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const deleteButtonRef = useRef<HTMLButtonElement>(null);
  const keepButtonRef = useRef<HTMLButtonElement>(null);
  const messageId = useId();
  useEffect(() => { if (confirmDelete) keepButtonRef.current?.focus(); }, [confirmDelete]);
  const keepPanel = () => { setConfirmDelete(false); requestAnimationFrame(() => deleteButtonRef.current?.focus()); };
  const name = draft.name.trim();
  const duplicate = panels.some((other) => other.id !== draft.id && other.name.trim().toLowerCase() === name.toLowerCase());
  const nameError = name === "" ? "Give the panel a name." : name.length > maxPanelNameChars ? `Panel names use ${maxPanelNameChars} characters or fewer.` : duplicate ? "Panel names must be unique." : null;
  const dirty = draft.name !== panel.name || draft.description !== panel.description;
  useEffect(() => { onDirtyChange(dirty); return () => onDirtyChange(false); }, [dirty, onDirtyChange]);
  const requestClose = () => { if (!dirty || window.confirm("Discard unsaved panel changes?")) onClose(); };
  return <div className="view panel-detail">
    <header className="page-header"><div><Button className="text-button panel-back" onClick={requestClose}><ChevronLeft size={18} aria-hidden="true" />All panels</Button><h1>{panel.name}</h1><p>Change the panel's name and description. Home shows the name under My panels.</p></div></header>
    {error && <p className="field-error" role="alert">{error}</p>}
    <div className="panel-detail-layout">
      <PanelPicture panel={draft} theme={theme} className="panel-picture-large" />
      <form className="panel-fields" onSubmit={(event) => { event.preventDefault(); if (!nameError) void onSave({ ...draft, name }); }}>
        <label className="field"><span>Panel name</span><Input value={draft.name} maxLength={maxPanelNameChars} aria-invalid={nameError ? true : undefined} aria-describedby={nameError ? "panel-name-error" : undefined} onChange={(event) => setDraft({ ...draft, name: event.target.value })} /></label>
        {nameError && <p id="panel-name-error" className="field-error">{nameError}</p>}
        <label className="field"><span>Description</span><Input value={draft.description} maxLength={maxPanelDescriptionChars} placeholder="What this panel is for" onChange={(event) => setDraft({ ...draft, description: event.target.value })} /></label>
        <div className="panel-actions">
          <Button className="primary" type="submit" disabled={busy || !dirty || !!nameError}>Save panel</Button>
          {confirmDelete
            ? <div className="panel-delete-confirm" role="alertdialog" aria-label="Delete this panel?" aria-describedby={messageId}><span id={messageId}>Delete this panel? Tiles on other panels that open it will say it no longer exists.</span><Button className="secondary danger" disabled={busy} onClick={() => void onDelete()}>Delete</Button><Button ref={keepButtonRef} className="secondary" onClick={keepPanel}>Keep</Button></div>
            : <Button ref={deleteButtonRef} className="secondary danger" disabled={busy} onClick={() => setConfirmDelete(true)}><Trash2 size={16} aria-hidden="true" />Delete panel</Button>}
        </div>
      </form>
    </div>
  </div>;
}
