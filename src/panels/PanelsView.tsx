import { useEffect, useRef, useState } from "react";
import { LayoutGrid, Plus } from "lucide-react";
import { Button } from "../ui/controls";
import { PanelPicture } from "./PanelPicture";
import { PanelEditor } from "./PanelEditor";
import type { PanelsController } from "./usePanels";
import type { PanelTheme } from "../scanning/preferences";
import type { CustomPanel } from "../types";

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
  if (open) return <PanelEditor key={open.id} panel={open} panels={panels ?? []} theme={theme} busy={busy} error={error} onDirtyChange={onDirtyChange} onClose={closePanel} onSave={(panel) => save(panel).then(closePanel, () => undefined)} onDelete={() => remove(open.id).then(closePanel, () => undefined)} />;
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
