// Finding things: the Find tile and its results, the section strip at the
// top of each page, the Also see links, Home's common tasks and recently
// changed, and the jump that takes a person to one setting and marks it.
import { useEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { useSemantic } from "./useSemantic";
import { Gauge, Keyboard, LayoutGrid, Maximize2, Palette, Search, Smartphone, X } from "lucide-react";
import { Button } from "../ui/controls";
import { available, commonTasks, entry, index, pageLabels, searchWithMeaning, sectionLabel, sections, targetOf, type Entry, type FindContext, type Page, type Target } from "./index";

export type Jump = (target: Target) => void;

const taskIcons = { "scan-rate": Gauge, colour: Palette, "switch-list": Keyboard, "panel-size": Maximize2, pairing: Smartphone, "your-panels": LayoutGrid };

/** A tile that takes you to one setting, showing where it is and what it is set to. */
export function JumpTile({ e, ctx, jump, task, raised }: { e: Entry; ctx: FindContext; jump: Jump; task?: string; raised?: boolean }) {
  const value = e.value(ctx);
  const where = `${pageLabels[e.page]}, ${sectionLabel(e.page, e.section)}`;
  const Icon = task ? taskIcons[e.id as keyof typeof taskIcons] : undefined;
  return <Button className={raised ? "tile tile-jump tile-raised" : "tile tile-jump"} onClick={() => jump(targetOf(e))} aria-label={`${task ?? e.label}, ${where}${value ? `, ${value}` : ""}`}>
    <span className="task-heading">{Icon && <span className="task-icon" aria-hidden="true"><Icon size={24} /></span>}<span className="tile-title">{task ?? e.label}</span></span>
    {task && <span className="tile-note">{e.label}</span>}
    <span className="tile-value-line" aria-hidden="true">{value ?? "Loading…"}</span>
    <span className="tile-where" aria-hidden="true">{where}</span>
  </Button>;
}

/** Compact section shortcuts. Accessible names retain the current values. */
export function SectionStrip({ page, ctx, jump }: { page: Page; ctx: FindContext; jump: Jump }) {
  const list = sections[page].filter((s) => !s.when || s.when(ctx));
  if (!list.length) return null;
  return <nav className="strip" aria-label="On this page">
    {list.map((s) => {
      const values = index.filter((e) => e.page === page && e.section === s.id && available(e, ctx)).map((e) => e.value(ctx)).filter((v): v is string => !!v);
      const summary = values.slice(0, 2).join(". ");
      return <Button key={s.id} className="tile tile-strip" onClick={() => jump({ page, section: s.id })} aria-label={summary ? `${s.label}, ${summary}` : s.label}>
        <span className="strip-label">{s.label}</span>
        <span className="strip-values" aria-hidden="true">{summary || "Open"}</span>
      </Button>;
    })}
  </nav>;
}

/** Settings on other pages that people often want next from this one. */
export function AlsoSee({ page, ctx, jump }: { page: Page; ctx: FindContext; jump: Jump }) {
  const ids = [...new Set(index.filter((e) => e.page === page && available(e, ctx)).flatMap((e) => e.related ?? []))].filter((id) => entry(id).page !== page && available(entry(id), ctx)).slice(0, 4);
  if (!ids.length) return null;
  return <section className="group" aria-labelledby="also-see-heading" data-section="also">
    <h2 id="also-see-heading">Also see</h2>
    <p className="group-note">Related settings on other pages.</p>
    <div className="grid">{ids.map((id) => <JumpTile key={id} e={entry(id)} ctx={ctx} jump={jump} raised />)}</div>
  </section>;
}

export function CommonTasks({ ctx, jump }: { ctx: FindContext; jump: Jump }) {
  return <section className="group" aria-labelledby="common-tasks-heading" data-section="tasks">
    <h2 id="common-tasks-heading">Common tasks</h2>
    <p className="group-note">Each one opens the exact setting.</p>
    <div className="grid">{commonTasks.filter((t) => available(entry(t.setting), ctx)).map((t) => <JumpTile key={t.setting} e={entry(t.setting)} ctx={ctx} jump={jump} task={t.label} />)}</div>
  </section>;
}

export function RecentlyChanged({ ctx, jump, recent }: { ctx: FindContext; jump: Jump; recent: string[] }) {
  return <section className="group" aria-labelledby="recent-heading" data-section="recent">
    <h2 id="recent-heading">Recently changed</h2>
    <p className="group-note">{recent.length ? "The last settings you changed, in case you want them back." : "Settings you change will be listed here so you can find them again."}</p>
    <div className="grid">
      {recent.map((id) => <JumpTile key={id} e={entry(id)} ctx={ctx} jump={jump} raised />)}
      {!recent.length && <div className="tile tile-raised tile-empty"><span className="tile-note">Nothing changed yet.</span></div>}
    </div>
  </section>;
}

/** Search in the workspace toolbar. */
export function FindTile({ query, onChange, inputRef, shortcut }: { query: string; onChange: (q: string) => void; inputRef: RefObject<HTMLInputElement | null>; shortcut?: string }) {
  // The whole tile is the target: a press anywhere on it focuses the box.
  return <div className={query ? "tile tile-find tile-chosen" : "tile tile-find"} role="search" onClick={() => inputRef.current?.focus()}>
    <Search size={24} aria-hidden="true" />
    <input ref={inputRef} type="search" aria-label="Find a setting" aria-keyshortcuts="Control+k Meta+k /" value={query} placeholder="Find a setting" onChange={(e) => onChange(e.target.value)} autoComplete="off" />
    {!query && shortcut && <kbd className="find-shortcut" aria-hidden="true">{shortcut}</kbd>}
    {query && <Button className="find-clear" aria-label="Clear search" onClick={() => { onChange(""); inputRef.current?.focus(); }}><X size={22} aria-hidden="true" /></Button>}
  </div>;
}

export function FindResults({ query, ctx, jump }: { query: string; ctx: FindContext; jump: Jump }) {
  const { semantic, loading } = useSemantic(query);
  const hits = searchWithMeaning(query, ctx, semantic);
  const title = hits.length ? `${hits.length} ${hits.length === 1 ? "setting matches" : "settings match"} “${query}”` : loading ? "Finding settings…" : `Nothing matches “${query}”`;
  return <div className="view">
    <p className="sr-only" role="status" aria-live="polite">{title}</p>
    <header className="page-header"><div>
      <h1>{title}</h1>
      <p>{hits.length ? "Choose one to go straight to it." : loading ? "Looking for settings that match what you mean." : "Try another word, such as speed, colour, phone or switch. Common tasks on Home may help too."}</p>
    </div></header>
    <div className="grid" role="list" aria-label="Matching settings">
      {hits.map((e) => <div key={e.id} role="listitem" className="grid-item"><JumpTile e={e} ctx={ctx} jump={jump} /></div>)}
      {!hits.length && !loading && commonTasks.slice(0, 4).map((t) => <div key={t.setting} role="listitem" className="grid-item"><JumpTile e={entry(t.setting)} ctx={ctx} jump={jump} task={t.label} raised /></div>)}
    </div>
  </div>;
}

/** The last few settings whose value changed, newest first, found by diffing the index. */
export function useRecent(ctx: FindContext | null): string[] {
  const [recent, setRecent] = useState<string[]>([]);
  const last = useRef<Record<string, string | null> | null>(null);
  useEffect(() => {
    if (!ctx) return;
    const now = Object.fromEntries(index.map((e) => [e.id, e.value(ctx)]));
    if (last.current) {
      // A value arriving for the first time is loading, not a change.
      const changed = index.filter((e) => !e.status && last.current![e.id] != null && now[e.id] != null && now[e.id] !== last.current![e.id]).map((e) => e.id);
      if (changed.length) setRecent((r) => [...changed, ...r.filter((id) => !changed.includes(id))].slice(0, 4));
    }
    last.current = now;
  }, [ctx]);
  return recent;
}

/** Opens any collapsed "More options" around an element so it can be seen. */
function reveal(el: HTMLElement) {
  for (let content = el.closest<HTMLElement>(".more-options-content"); content; content = content.parentElement?.closest<HTMLElement>(".more-options-content") ?? null) {
    if (content.hidden) content.closest(".more-options")?.querySelector<HTMLButtonElement>(".more-options-toggle")?.click();
  }
}

/**
 * After a jump, finds the setting (or its section) once the page has
 * rendered it, scrolls there, focuses it and marks it found for a moment.
 * Keeps looking for a second, since a page or scanning area may still be
 * on its way.
 */
export function useArrival(root: RefObject<HTMLElement | null>, target: Target | null, done: () => void) {
  useEffect(() => {
    if (!target) return;
    let tries = 0;
    let lookTimer = 0;
    let found: HTMLElement | null = null;
    const look = () => {
      const el = (target.setting && root.current?.querySelector<HTMLElement>(`[data-setting="${target.setting}"]`)) || root.current?.querySelector<HTMLElement>(`[data-section="${target.section}"]`) || null;
      if (!el || (target.setting && !el.matches(`[data-setting="${target.setting}"]`) && tries < 10)) {
        if (++tries < 40) { lookTimer = window.setTimeout(look, 25); return; }
        if (!el) { done(); return; }
      }
      found = el!;
      // Opening a collapsed group re-renders on the next tick, and a hidden
      // element can be neither scrolled to nor focused, so look again then.
      if (found.closest<HTMLElement>(".more-options-content")?.hidden) {
        reveal(found);
        if (++tries < 40) { lookTimer = window.setTimeout(look, 25); return; }
      }
      const reduce = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? true;
      found.scrollIntoView?.({ block: target.setting ? "center" : "start", behavior: reduce ? "auto" : "smooth" });
      const focusable = found.matches("button, select, input, textarea, [tabindex]") ? found : found.querySelector<HTMLElement>("h2, h3, button:not(:disabled), select, input, textarea, [tabindex]");
      if (focusable && !focusable.hasAttribute("tabindex") && !focusable.matches("button, select, input, textarea, a")) focusable.setAttribute("tabindex", "-1");
      focusable?.focus({ preventScroll: true });
      found.setAttribute("data-found", "true");
      // The mark outlives this effect: `done` clears the target straight away.
      window.setTimeout(() => found?.removeAttribute("data-found"), 2600);
      done();
    };
    look();
    return () => window.clearTimeout(lookTimer);
  }, [target]);
}

/** A page's standard header and strip. */
export function PageHead({ title, blurb, action }: { title: string; blurb: ReactNode; action?: ReactNode }) {
  return <header className="page-header"><div><h1>{title}</h1><p>{blurb}</p></div>{action}</header>;
}
