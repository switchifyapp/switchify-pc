import { FeedbackMessage, PageHeader, SectionHeader, ActionRow } from "../ui/patterns";
import { PageIcon } from "../ui/PageIcon";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { Plus } from "lucide-react";
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

/** What a native panel's row says about the tiles around its keys. */
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
 * Where each built-in menu is reached from, so the page can list it under
 * the menu that opens it. Menus on both Home and More are listed once,
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

/** The summary as the row shows it: each part capitalised, with a dot between. */
function sentence(text: string) {
  return text.split(", ").map((part) => part.charAt(0).toUpperCase() + part.slice(1)).join(" · ");
}

export function PanelsView({ controller, theme, onDirtyChange, strip, alsoSee }: { controller: PanelsController; theme?: PanelTheme; onDirtyChange: (dirty: boolean) => void; strip?: ReactNode; alsoSee?: ReactNode }) {
  const { panels, builtins, extras, busy, error, clearError, save, remove, saveBuiltin, resetBuiltin, saveExtras, tryPanel, setEditing } = controller;
  const [open, setOpen] = useState<CustomPanel | null>(null);
  const [builtin, setBuiltin] = useState<BuiltinEditing | null>(null);
  const [extra, setExtra] = useState<ExtrasEditing | null>(null);
  // The card or row that opened the editor, found again by id once the list is back.
  const openerRef = useRef<string | null>(null);
  // Leaving the page by any route lets sync refresh the list again.
  useEffect(() => () => setEditing(false), [setEditing]);
  const openPanel = (panel: CustomPanel, opener: string | null) => { openerRef.current = opener; clearError(); setEditing(true); setBuiltin(null); setExtra(null); setOpen(panel); };
  const openBuiltin = (menu: BuiltinPanelView) => { openerRef.current = `builtin-${menu.kind}`; clearError(); setEditing(true); setBuiltin({ kind: menu.kind, rearranged: menu.override !== null }); setExtra(null); setOpen(menu.override ?? menu.default); };
  const openExtras = (view: PanelExtrasView) => { openerRef.current = `extras-${view.target}`; clearError(); setEditing(true); setBuiltin(null); setExtra({ target: view.target, fixed: view.fixed, columns: view.columns, hasTiles: view.rows.length > 0 || view.columns.length > 0 }); setOpen(extrasPanel(view)); };
  const closePanel = () => {
    setEditing(false);
    setOpen(null);
    setBuiltin(null);
    setExtra(null);
    requestAnimationFrame(() => (document.querySelector<HTMLButtonElement>(`[data-opener="${openerRef.current ?? "new-panel"}"]`) ?? document.querySelector<HTMLButtonElement>('[data-opener="new-panel"]'))?.focus());
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
  // The built-in menus and panels are a list, one row each, under the menu that opens them.
  const menu = (kind: string) => builtins.find((candidate) => candidate.kind === kind);
  const row = (id: string, name: string, summary: string, onClick: () => void) => <ActionRow className="menu-row" data-opener={id} onClick={onClick} aria-label={`${name}, ${summary}`} title={name} description={sentence(summary)} />;
  const builtinItem = (kind: string): ReactNode => {
    const view = menu(kind);
    if (!view) return null;
    const inside = builtins.filter((candidate) => INSIDE[candidate.kind] === kind);
    return <li key={kind}>
      {row(`builtin-${kind}`, view.name, builtinSummary(view), () => openBuiltin(view))}
      {inside.length > 0 && <ul className="menu-list-inside" aria-label={`Inside ${view.name}`}>{inside.map((child) => <li key={child.kind}>{row(`builtin-${child.kind}`, child.name, builtinSummary(child), () => openBuiltin(child))}</li>)}</ul>}
    </li>;
  };
  const extrasItem = (view: PanelExtrasView) => <li key={view.target}>{row(`extras-${view.target}`, view.name, extrasSummary(view), () => openExtras(view))}</li>;
  // A menu inside another counts as placed only while its parent is there to list it under.
  const placed = new Set(["home", "more", ...HOME_MENUS, ...MORE_MENUS, ...Object.keys(INSIDE).filter((kind) => menu(INSIDE[kind]))]);
  const unplaced = builtins.filter((candidate) => !placed.has(candidate.kind));
  const onHome = menu("home") !== undefined || extras.length > 0 || HOME_MENUS.some(menu);
  const onMore = menu("more") !== undefined || MORE_MENUS.some(menu);
  return <div className="view">
    <PageHeader title="Panels" description="Panels of your own, which Home lists under My panels, then the built-in menus and panels." action={<Button data-opener="new-panel" className="primary" disabled={busy || panels === null} onClick={() => void create()}><Plus size={16} />New panel</Button>} />
    {strip}
    {error && <FeedbackMessage>{error}</FeedbackMessage>}
    <section className="panel-group panel-group-yours" data-section="yours" data-setting="your-panels" aria-label="Your panels">
    {panels === null ? <p className="capability-note">Loading your panels...</p>
      : panels.length === 0 ? <section className="empty-state" aria-label="No panels yet"><PageIcon page="panels" size={32} /><h2>Your first panel starts here</h2><p>Put the actions you use together on one panel. Start with Keyboard, Mouse, Back and Close, then make it your own.</p><Button className="primary" disabled={busy} onClick={() => void create()}>Create your first panel</Button></section>
      : <ul className="panel-grid" aria-label="Your panels">{panels.map((panel) => <li key={panel.id}>
        <Button className="panel-card" data-opener={panel.id} onClick={() => openPanel(panel, panel.id)} aria-label={`${panel.name}, ${tileCount(panel)} tiles`}>
          <PanelPicture panel={panel} theme={theme} />
          <span className="panel-card-text"><strong>{panel.name}</strong><small>{panel.description.trim() || `${tileCount(panel)} ${tileCount(panel) === 1 ? "tile" : "tiles"}`}</small></span>
        </Button>
      </li>)}</ul>}
    </section>
    {(onHome || onMore || unplaced.length > 0) && <section className="panel-group" data-section="builtin" data-setting="builtin-menus" aria-labelledby="builtin-heading">
      <SectionHeader headingId="builtin-heading" title="Built-in menus and panels" description="Listed under the menu that opens them. More actions is a tile on the page that opens after you choose a point, and its menu opens most of Home's menus too. Rearrange a menu or put it back the way it came, or add tiles of your own around the mouse panel's and the keyboard's keys." />
      <div className="menu-lists">
        {onHome && <section>
          <h3 id="home-list-heading">On Home</h3>
          <ul className="menu-list" aria-labelledby="home-list-heading">
            {builtinItem("home")}
            {extras.map(extrasItem)}
            {HOME_MENUS.map(builtinItem)}
          </ul>
        </section>}
        {onMore && <section>
          <h3 id="more-list-heading">On More actions</h3>
          <ul className="menu-list" aria-labelledby="more-list-heading">
            {builtinItem("more")}
            {MORE_MENUS.map(builtinItem)}
          </ul>
        </section>}
        {unplaced.length > 0 && <section>
          <h3 id="other-list-heading">Other menus</h3>
          <ul className="menu-list" aria-labelledby="other-list-heading">{unplaced.map((view) => builtinItem(view.kind))}</ul>
        </section>}
      </div>
    </section>}
    {alsoSee}
  </div>;
}
