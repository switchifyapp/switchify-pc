import { useEffect, useMemo, useRef, useState } from "react";
import { Settings } from "lucide-react";
import type { AppSettings, AppState } from "../types";
import { Tabs, TabPanel, type TabDefinition } from "../Tabs";
import { GeneralSection } from "./GeneralSection";
import { PointerSection } from "./PointerSection";
import { CursorSection } from "./CursorSection";
import { PrivacySection } from "./PrivacySection";
import { UpdatesSection, type UpdateAction } from "./UpdatesSection";

import { SwitchesSection } from "./SwitchesSection";
import type { SwitchController } from "../scanning/useSwitches";
import { ScanningSection } from "./ScanningSection";
import type { ScanningController } from "../scanning/useScanning";

type SettingsTabId = "general" | "switches" | "scanning" | "pointer" | "cursor" | "privacy" | "updates";

export function SettingsView({ switches, scanning, state, settings, onChange, chooseTelemetry, updateAction, cancelUpdate, busy, focusUpdates, onUpdatesFocused, updateAttention, onUpdatesShown }: { switches: SwitchController; scanning: ScanningController; state: AppState; settings: AppSettings; onChange: (next: AppSettings) => void; chooseTelemetry: (enabled: boolean) => void; updateAction: (action: UpdateAction) => void; cancelUpdate: () => void; busy: boolean; focusUpdates: boolean; onUpdatesFocused: () => void; updateAttention: string | null; onUpdatesShown: (shown: boolean) => void }) {
  const updatesRef = useRef<HTMLElement>(null);
  // Opening straight to Updates starts there, rather than committing General
  // for one frame and letting App announce a failure for a tab already being
  // opened.
  const [active, setActive] = useState<SettingsTabId>(focusUpdates ? "updates" : "general");

  // App owns the standing update failure and what has been said about it; this
  // view only reports whether the Updates panel, which shows it, is on screen.
  useEffect(() => {
    onUpdatesShown(active === "updates");
    return () => onUpdatesShown(false);
  }, [active, onUpdatesShown]);

  const tabs = useMemo<TabDefinition<SettingsTabId>[]>(() => [
    { id: "general" as const, label: "General" },
    { id: "pointer" as const, label: "Controls" },
    { id: "switches" as const, label: "Switches" },
    { id: "scanning" as const, label: "Scanning" },
    ...(state.capabilities.cursorOverlay ? [{ id: "cursor" as const, label: "Cursor appearance" }] : []),
    { id: "privacy" as const, label: "Privacy" },
    // On the Updates tab the panel itself shows the reason, so no marker there.
    { id: "updates" as const, label: "Updates", attention: updateAttention && active !== "updates" ? updateAttention : undefined },
  ], [state.capabilities.cursorOverlay, updateAttention, active]);

  useEffect(() => {
    if (!tabs.some((tab) => tab.id === active)) setActive("general");
  }, [tabs, active]);

  useEffect(() => {
    if (focusUpdates) setActive("updates");
  }, [focusUpdates]);

  useEffect(() => {
    if (!focusUpdates || active !== "updates") return;
    updatesRef.current?.scrollIntoView?.({ block: "start" });
    updatesRef.current?.focus({ preventScroll: true });
    onUpdatesFocused();
  }, [focusUpdates, active, onUpdatesFocused]);

  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => onChange({ ...settings, [key]: value });

  return <div className="view settings-view"><header className="page-header"><div><h1>Settings</h1><p>How Switchify PC behaves on this computer</p></div><Settings size={24} /></header>
    <Tabs name="settings" tabs={tabs} active={active} onSelect={setActive} label="Settings sections" />
    <TabPanel name="settings" id={active}>
      {active === "general" && <GeneralSection settings={settings} update={update} />}
      {active === "pointer" && <PointerSection settings={settings} update={update} />}
      {active === "switches" && <SwitchesSection controller={switches} />}
      {active === "scanning" && <ScanningSection controller={scanning} />}
      {active === "cursor" && <CursorSection settings={settings} update={update} />}
      {active === "privacy" && <PrivacySection state={state} settings={settings} update={update} chooseTelemetry={chooseTelemetry} busy={busy} />}
      {active === "updates" && <UpdatesSection state={state} run={updateAction} cancel={cancelUpdate} sectionRef={updatesRef} />}
    </TabPanel>
  </div>;
}
