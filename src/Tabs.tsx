import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";

// `attention` is the reason a tab needs it. It adds a visual marker without
// changing the tab's accessible name, and is attached as the tab's description,
// so assistive tech hears why while tests and speech input keep the stable name.
export type TabDefinition<T extends string = string> = { id: T; label: string; attention?: string };

// `name` scopes the element ids, so two tablists can share a page.
function tabId(name: string, id: string) { return `${name}-tab-${id}`; }
function panelId(name: string, id: string) { return `${name}-panel-${id}`; }
function descriptionId(name: string, id: string) { return `${tabId(name, id)}-description`; }

const TABBABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function Tabs<T extends string>({ name, tabs, active, onSelect, label }: { name: string; tabs: readonly TabDefinition<T>[]; active: T; onSelect: (id: T) => void; label: string }) {
  const listRef = useRef<HTMLDivElement>(null);
  // The tab stop stays on the last tab that held focus, so arrowing to a tab and
  // then tabbing away and back returns to it rather than to the selected tab.
  const [focused, setFocused] = useState<T | null>(null);
  const tabStop = tabs.some((tab) => tab.id === focused) ? focused : active;
  // Selecting a tab — by click or programmatically, as the update banner does —
  // moves the stop back onto the selection, so it can never strand the tab stop
  // on a tab that is no longer selected.
  useEffect(() => { setFocused(null); }, [active]);

  const focusTab = (index: number) => {
    const buttons = listRef.current?.querySelectorAll<HTMLButtonElement>('[role="tab"]');
    buttons?.[(index + tabs.length) % tabs.length]?.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    if (event.key === "ArrowRight") focusTab(index + 1);
    else if (event.key === "ArrowLeft") focusTab(index - 1);
    else if (event.key === "Home") focusTab(0);
    else if (event.key === "End") focusTab(tabs.length - 1);
    else return;
    event.preventDefault();
  };

  return <>
    <div className="segmented tab-bar" role="tablist" aria-label={label} ref={listRef} style={{ ["--tab-count" as string]: tabs.length }}>
      {tabs.map((tab, index) => <button
        key={tab.id}
        type="button"
        role="tab"
        id={tabId(name, tab.id)}
        aria-selected={active === tab.id}
        // Only the selected panel is rendered, so pointing inactive tabs at absent
        // ids would leave dangling IDREFs.
        aria-controls={active === tab.id ? panelId(name, tab.id) : undefined}
        aria-describedby={tab.attention ? descriptionId(name, tab.id) : undefined}
        tabIndex={tabStop === tab.id ? 0 : -1}
        onFocus={() => setFocused(tab.id)}
        onKeyDown={(event) => onKeyDown(event, index)}
        onClick={() => onSelect(tab.id)}
      >{tab.label}{tab.attention && <span className="tab-attention" aria-hidden="true" />}</button>)}
    </div>
    {/* Outside the tablist so the reason never becomes part of a tab's name. */}
    {tabs.filter((tab) => tab.attention).map((tab) => <span key={tab.id} id={descriptionId(name, tab.id)} className="sr-only">{tab.attention}</span>)}
  </>;
}

export function TabPanel<T extends string>({ name, id, children }: { name: string; id: T; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  // A panel is only its own tab stop when nothing inside it can take focus,
  // which the Updates panel hits while a check or install is in flight.
  const [tabbable, setTabbable] = useState(false);
  useEffect(() => {
    // Keep the stop while the panel itself holds focus: removing tabIndex from
    // the focused element would drop focus to the document body.
    setTabbable((current) => !ref.current?.querySelector(TABBABLE) || (current && ref.current === document.activeElement));
  });
  return <div ref={ref} className="tab-panel" role="tabpanel" id={panelId(name, id)} aria-labelledby={tabId(name, id)} tabIndex={tabbable ? 0 : undefined}>{children}</div>;
}
