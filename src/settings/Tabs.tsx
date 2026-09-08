import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";

// `attention` adds a visual marker without changing the tab's accessible name;
// `describedBy` points at the element that says why, so assistive tech gets the
// reason without tests and speech input losing the stable name.
export type TabDefinition = { id: string; label: string; attention?: boolean; describedBy?: string };

export function tabId(id: string) { return `settings-tab-${id}`; }
export function panelId(id: string) { return `settings-panel-${id}`; }

const TABBABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function Tabs({ tabs, active, onSelect, label }: { tabs: readonly TabDefinition[]; active: string; onSelect: (id: string) => void; label: string }) {
  const listRef = useRef<HTMLDivElement>(null);
  // The tab stop stays on the last tab that held focus, so arrowing to a tab and
  // then tabbing away and back returns to it rather than to the selected tab.
  const [focused, setFocused] = useState<string | null>(null);
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

  return <div className="segmented settings-tabs" role="tablist" aria-label={label} ref={listRef} style={{ ["--tab-count" as string]: tabs.length }}>
    {tabs.map((tab, index) => <button
      key={tab.id}
      type="button"
      role="tab"
      id={tabId(tab.id)}
      aria-selected={active === tab.id}
      // Only the selected panel is rendered, so pointing inactive tabs at absent
      // ids would leave dangling IDREFs.
      aria-controls={active === tab.id ? panelId(tab.id) : undefined}
      aria-describedby={tab.describedBy}
      tabIndex={tabStop === tab.id ? 0 : -1}
      onFocus={() => setFocused(tab.id)}
      onKeyDown={(event) => onKeyDown(event, index)}
      onClick={() => onSelect(tab.id)}
    >{tab.label}{tab.attention && <span className="tab-attention" aria-hidden="true" />}</button>)}
  </div>;
}

export function TabPanel({ id, children }: { id: string; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  // A panel is only its own tab stop when nothing inside it can take focus,
  // which the Updates panel hits while a check or install is in flight.
  const [tabbable, setTabbable] = useState(false);
  useEffect(() => {
    // Keep the stop while the panel itself holds focus: removing tabIndex from
    // the focused element would drop focus to the document body.
    setTabbable((current) => !ref.current?.querySelector(TABBABLE) || (current && ref.current === document.activeElement));
  });
  return <div ref={ref} className="settings-panel" role="tabpanel" id={panelId(id)} aria-labelledby={tabId(id)} tabIndex={tabbable ? 0 : undefined}>{children}</div>;
}
