import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";

export type TabDefinition = { id: string; label: string };

export function tabId(id: string) { return `settings-tab-${id}`; }
export function panelId(id: string) { return `settings-panel-${id}`; }

const TABBABLE = 'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function Tabs({ tabs, active, onSelect, label }: { tabs: readonly TabDefinition[]; active: string; onSelect: (id: string) => void; label: string }) {
  const listRef = useRef<HTMLDivElement>(null);
  // The tab stop stays on the last tab that held focus, so arrowing to a tab and
  // then tabbing away and back returns to it rather than to the selected tab.
  const [focused, setFocused] = useState<string | null>(null);
  const tabStop = tabs.some((tab) => tab.id === focused) ? focused : active;

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
      tabIndex={tabStop === tab.id ? 0 : -1}
      onFocus={() => setFocused(tab.id)}
      onKeyDown={(event) => onKeyDown(event, index)}
      onClick={() => { setFocused(tab.id); onSelect(tab.id); }}
    >{tab.label}</button>)}
  </div>;
}

export function TabPanel({ id, children }: { id: string; children: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  // A panel is only its own tab stop when nothing inside it can take focus,
  // which the Updates panel hits while a check or install is in flight.
  const [tabbable, setTabbable] = useState(false);
  useEffect(() => {
    setTabbable(!ref.current?.querySelector(TABBABLE));
  });
  return <div ref={ref} className="settings-panel" role="tabpanel" id={panelId(id)} aria-labelledby={tabId(id)} tabIndex={tabbable ? 0 : undefined}>{children}</div>;
}
