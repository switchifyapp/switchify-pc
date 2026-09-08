import { useRef, type KeyboardEvent, type ReactNode } from "react";

export type TabDefinition = { id: string; label: string };

export function tabId(id: string) { return `settings-tab-${id}`; }
export function panelId(id: string) { return `settings-panel-${id}`; }

export function Tabs({ tabs, active, onSelect, label }: { tabs: readonly TabDefinition[]; active: string; onSelect: (id: string) => void; label: string }) {
  const listRef = useRef<HTMLDivElement>(null);

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
      aria-controls={panelId(tab.id)}
      tabIndex={active === tab.id ? 0 : -1}
      onKeyDown={(event) => onKeyDown(event, index)}
      onClick={() => onSelect(tab.id)}
    >{tab.label}</button>)}
  </div>;
}

export function TabPanel({ id, children }: { id: string; children: ReactNode }) {
  return <div className="settings-panel" role="tabpanel" id={panelId(id)} aria-labelledby={tabId(id)} tabIndex={0}>{children}</div>;
}
