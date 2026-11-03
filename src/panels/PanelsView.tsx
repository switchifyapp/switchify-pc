import { useEffect, useRef, useState, type ReactNode } from "react";
import { LayoutGrid, Plus } from "lucide-react";
import { Button } from "../ui/controls";
import { PanelPicture } from "./PanelPicture";
import { PanelEditor, type BuiltinEditing, type ExtrasEditing } from "./PanelEditor";
import type { PanelsController } from "./usePanels";
import type { PanelTheme } from "../scanning/preferences";
import type { BuiltinPanelView, CustomPanel, PanelExtrasView } from "../types";

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

/** What a native panel's card says about the tiles around its keys. */
export function extrasSummary(view: PanelExtrasView) {
  const count = (n: number, noun: string) => `${n} ${noun}${n === 1 ? "" : "s"}`;
  const parts = [view.rows.length > 0 && count(view.rows.length, "extra row"), view.columns.length > 0 && count(view.columns.length, "column")].filter((part): part is string => Boolean(part));
  return parts.length === 0 ? "keys only" : parts.join(" and ");
}

/** The rows below a native panel's keys, in the shape the editor edits. */
function extrasPanel(view: PanelExtrasView): CustomPanel {
  return { id: `extras-${view.target}`, version: 1, name: view.name, description: "", rows: view.rows };
}

/**
 * Where each built-in menu is reached from, so the page can list them under
 * the menu that opens them. Menus on both Home and More are listed once,
 * under Home, and say so; Tabs sits inside Browser.
 */
const HOME_MENUS = ["windows", "editing", "browser", "media", "zoom"];
const MORE_MENUS = ["mouse", "displays", "scanning"];
const ALSO_ON_MORE = new Set(["windows", "editing", "browser", "media", "zoom"]);
const INSIDE: Record<string, string> = { tabs: "browser" };

/** The small text under a built-in menu's name: its state, then where else it lives. */
export function builtinSummary(menu: BuiltinPanelView) {
  const state = menu.override ? "rearranged" : "not changed";
  const place = INSIDE[menu.kind] ? "in Browser" : ALSO_ON_MORE.has(menu.kind) ? "also on More" : null;
  return place ? `${state}, ${place}` : state;
}

/** The summary as the card shows it: each part capitalised, with a dot between. */
function sentence(text: string) {
  return text.split(", ").map((part) => part.charAt(0).toUpperCase() + part.slice(1)).join(" · ");
}

export function PanelsView({ controller, theme, onDirtyChange }: { controller: PanelsController; theme?: PanelTheme; onDirtyChange: (dirty: boolean) => void }) {
  const { panels, builtins, extras, busy, error, clearError, save, remove, saveBuiltin, resetBuiltin, saveExtras, tryPanel, setEditing } = controller;
  const [open, setOpen] = useState<CustomPanel | null>(null);
  const [builtin, setBuiltin] = useState<BuiltinEditing | null>(null);
  const [extra, setExtra] = useState<ExtrasEditing | null>(null);
  const openerRef = useRef<HTMLButtonElement | null>(null);
  // Leaving the page by any route lets sync refresh the list again.
  useEffect(() => () => setEditing(false), [setEditing]);
  const openPanel = (panel: CustomPanel, opener: HTMLButtonElement | null) => { openerRef.current = opener; clearError(); setEditing(true); setBuiltin(null); setExtra(null); setOpen(panel); };
  const openBuiltin = (menu: BuiltinPanelView, opener: HTMLButtonElement) => { openerRef.current = opener; clearError(); setEditing(true); setBuiltin({ kind: menu.kind, rearranged: menu.override !== null }); setExtra(null); setOpen(menu.override ?? menu.default); };
  const openExtras = (view: PanelExtrasView, opener: HTMLButtonElement) => { openerRef.current = opener; clearError(); setEditing(true); setBuiltin(null); setExtra({ target: view.target, fixed: view.fixed, columns: view.columns, hasTiles: view.rows.length > 0 || view.columns.length > 0 }); setOpen(extrasPanel(view)); };
  const closePanel = () => {
    setEditing(false);
    setOpen(null);
    setBuiltin(null);
    setExtra(null);
    requestAnimationFrame(() => (openerRef.current?.isConnected ? openerRef.current : document.querySelector<HTMLButtonElement>(".page-header button"))?.focus());
  };
  const create = async () => {
    const panel = starterPanel(panels ?? []);
    const saved = await save(panel).then((list) => list.find((candidate) => candidate.id === panel.id) ?? panel, () => null);
    if (saved === null) return;
    openPanel(saved, null);
  };
  if (open && extra) return <PanelEditor key={open.id} panel={open} panels={panels ?? []} theme={theme} busy={busy} error={error} extras={extra} onDirtyChange={onDirtyChange} onClearError={clearError} onClose={closePanel} onSave={(panel) => saveExtras(extra.target, panel.rows, extra.columns).then(closePanel, () => undefined)} onSaveExtras={(rows, columns) => saveExtras(extra.target, rows, columns).then(closePanel, () => undefined)} onDelete={() => saveExtras(extra.target, [], []).then(closePanel, () => undefined)} onTry={() => tryPanel({ kind: "native", panel: extra.target as "mouse" | "keyboard" }).then(() => undefined, () => undefined)} />;
  if (open && builtin) return <PanelEditor key={open.id} panel={open} panels={panels ?? []} theme={theme} busy={busy} error={error} builtin={builtin} onDirtyChange={onDirtyChange} onClearError={clearError} onClose={closePanel} onSave={(panel) => saveBuiltin(builtin.kind, panel).then(closePanel, () => undefined)} onDelete={() => resetBuiltin(builtin.kind).then(closePanel, () => undefined)} onTry={() => tryPanel({ kind: "menu", menu: builtin.kind }).then(() => undefined, () => undefined)} />;
  if (open) return <PanelEditor key={open.id} panel={open} panels={panels ?? []} theme={theme} busy={busy} error={error} onDirtyChange={onDirtyChange} onClearError={clearError} onClose={closePanel} onSave={(panel) => save(panel).then(closePanel, () => undefined)} onDelete={() => remove(open.id).then(closePanel, () => undefined)} onTry={() => tryPanel({ kind: "panel", id: open.id }).then(() => undefined, () => undefined)} />;
  const menu = (kind: string) => builtins.find((candidate) => candidate.kind === kind);
  const builtinCard = (view: BuiltinPanelView) => <Button className="panel-card" onClick={(event) => openBuiltin(view, event.currentTarget)} aria-label={`${view.name}, ${builtinSummary(view)}`}>
    <PanelPicture panel={view.override ?? view.default} theme={theme} />
    <span className="panel-card-text"><strong>{view.name}</strong><small>{sentence(builtinSummary(view))}</small></span>
  </Button>;
  // A menu's card, with the cards of the menus inside it listed under it.
  const builtinItem = (kind: string): ReactNode => {
    const view = menu(kind);
    if (!view) return null;
    const inside = builtins.filter((candidate) => INSIDE[candidate.kind] === kind);
    return <li key={kind}>
      {builtinCard(view)}
      {inside.length > 0 && <ul className="panel-grid-inside" aria-label={`Inside ${view.name}`}>{inside.map((child) => <li key={child.kind}>{builtinCard(child)}</li>)}</ul>}
    </li>;
  };
  const extrasItem = (view: PanelExtrasView) => <li key={view.target}>
    <Button className="panel-card" onClick={(event) => openExtras(view, event.currentTarget)} aria-label={`${view.name}, ${extrasSummary(view)}`}>
      <PanelPicture panel={extrasPanel(view)} fixed={view.fixed} columns={view.columns} theme={theme} />
      <span className="panel-card-text"><strong>{view.name}</strong><small>{extrasSummary(view).replace(/^keys/, "Keys")}</small></span>
    </Button>
  </li>;
  const placed = new Set(["home", "more", ...HOME_MENUS, ...MORE_MENUS, ...Object.keys(INSIDE)]);
  const unplaced = builtins.filter((candidate) => !placed.has(candidate.kind));
  const home = menu("home");
  const more = menu("more");
  return <div className="view">
    <header className="page-header"><div><h1>Panels</h1><p>Home, the menus it opens, panels of your own, and the mouse panel and keyboard. Choose one to change it; each is drawn as it scans.</p></div></header>
    {error && <p className="field-error" role="alert">{error}</p>}
    {(home || extras.length > 0 || HOME_MENUS.some(menu)) && <section className="panel-group" aria-labelledby="home-group-heading">
      <header className="section-header"><div><h2 id="home-group-heading">Home</h2><p>Scanning starts here. Home opens the mouse panel, the keyboard and these menus. Rearrange a menu or put it back the way it came; add tiles of your own around the mouse panel's and the keyboard's keys.</p></div></header>
      <ul className="panel-grid">
        {home && builtinItem("home")}
        {extras.map(extrasItem)}
        {HOME_MENUS.map(builtinItem)}
      </ul>
    </section>}
    <section className="panel-group" aria-labelledby="my-panels-heading">
      <header className="section-header"><div><h2 id="my-panels-heading">My panels</h2><p>Panels of your own. Home lists them under My panels.</p></div><Button className="primary" disabled={busy || panels === null} onClick={() => void create()}><Plus size={16} />New panel</Button></header>
      {panels === null ? <p className="capability-note">Loading your panels...</p>
        : panels.length === 0 ? <section className="empty-state" aria-label="No panels yet"><LayoutGrid size={32} aria-hidden="true" /><h2>No panels yet</h2><p>A panel is a page of tiles that scans like Home. Each tile presses keys, types a phrase, runs an action or opens another panel. Make one and it appears in Home under My panels.</p></section>
        : <ul className="panel-grid" aria-label="Your panels">{panels.map((panel) => <li key={panel.id}>
          <Button className="panel-card" onClick={(event) => openPanel(panel, event.currentTarget)} aria-label={`${panel.name}, ${tileCount(panel)} tiles`}>
            <PanelPicture panel={panel} theme={theme} />
            <span className="panel-card-text"><strong>{panel.name}</strong><small>{panel.description.trim() || `${tileCount(panel)} ${tileCount(panel) === 1 ? "tile" : "tiles"}`}</small></span>
          </Button>
        </li>)}</ul>}
    </section>
    {(more || MORE_MENUS.some(menu)) && <section className="panel-group" aria-labelledby="more-group-heading">
      <header className="section-header"><div><h2 id="more-group-heading">More actions</h2><p>After you choose a point, More on the Actions page opens this menu. It opens most of Home's menus too, and these three of its own.</p></div></header>
      <ul className="panel-grid">
        {more && builtinItem("more")}
        {MORE_MENUS.map(builtinItem)}
      </ul>
    </section>}
    {unplaced.length > 0 && <section className="panel-group" aria-labelledby="other-menus-heading">
      <header className="section-header"><div><h2 id="other-menus-heading">Other menus</h2></div></header>
      <ul className="panel-grid">{unplaced.map((view) => builtinItem(view.kind))}</ul>
    </section>}
  </div>;
}
