import { useRef, useState, type FocusEvent, type KeyboardEvent, type ReactNode } from "react";

export type TabDefinition = { id: string; label: string };

export function tabId(id: string) { return `settings-tab-${id}`; }
export function panelId(id: string) { return `settings-panel-${id}`; }

export function Tabs({ tabs, active, onSelect, label }: { tabs: readonly TabDefinition[]; active: string; onSelect: (id: string) => void; label: string }) {
  const listRef = useRef<HTMLDivElement>(null);
  // The tab stop follows focus while the tablist has it, and falls back to the
  // selected tab once focus leaves, so arrowing to a tab and tabbing away and
  // back does not discard the user's position.
  const [focused, setFocused] = useState<string | null>(null);
  const tabStop = focused ?? active;

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

  const onBlur = (event: FocusEvent<HTMLDivElement>) => {
    if (!event.currentTarget.contains(event.relatedTarget)) setFocused(null);
  };

  return <div className="segmented settings-tabs" role="tablist" aria-label={label} ref={listRef} onBlur={onBlur} style={{ ["--tab-count" as string]: tabs.length }}>
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
  // No tabIndex: every panel contains focusable controls, so making the panel
  // itself tabbable would add a redundant stop before the first real control.
  return <div className="settings-panel" role="tabpanel" id={panelId(id)} aria-labelledby={tabId(id)}>{children}</div>;
}
