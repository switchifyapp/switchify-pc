import { useEffect, useMemo, useRef, useState } from "react";
import { Settings } from "lucide-react";
import type { AppSettings, AppState } from "../types";
import { Tabs, TabPanel } from "./Tabs";
import { GeneralSection } from "./GeneralSection";
import { PointerSection } from "./PointerSection";
import { CursorSection } from "./CursorSection";
import { PrivacySection } from "./PrivacySection";
import { UpdatesSection, updateDescription, type UpdateAction } from "./UpdatesSection";

const updatesNoticeId = "settings-updates-notice";

export function SettingsView({ state, settings, onChange, chooseTelemetry, updateAction, cancelUpdate, busy, focusUpdates, onUpdatesFocused }: { state: AppState; settings: AppSettings; onChange: (next: AppSettings) => void; chooseTelemetry: (enabled: boolean) => void; updateAction: (action: UpdateAction) => void; cancelUpdate: () => void; busy: boolean; focusUpdates: boolean; onUpdatesFocused: () => void }) {
  const updatesRef = useRef<HTMLElement>(null);
  const [active, setActive] = useState("general");

  // Failed and cancelled are the only updater states with something to act on
  // that the global banner deliberately does not show. Their status line lives
  // in the Updates panel, which is unmounted on other tabs, so surface them
  // here: a marker on the tab, and a live region so the change is announced.
  const updaterNeedsAttention = state.updater.status === "failed" || state.updater.status === "cancelled";
  const showUpdatesNotice = updaterNeedsAttention && active !== "updates";

  const tabs = useMemo(() => [
    { id: "general", label: "General" },
    { id: "pointer", label: "Pointer" },
    ...(state.capabilities.cursorOverlay ? [{ id: "cursor", label: "Cursor" }] : []),
    { id: "privacy", label: "Privacy" },
    { id: "updates", label: "Updates", attention: updaterNeedsAttention, describedBy: showUpdatesNotice ? updatesNoticeId : undefined },
  ], [state.capabilities.cursorOverlay, updaterNeedsAttention, showUpdatesNotice]);

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

  return <div className="view"><header className="page-header"><div><h1>Settings</h1><p>How Switchify PC behaves on this computer</p></div><Settings size={24} /></header>
    <Tabs tabs={tabs} active={active} onSelect={setActive} label="Settings sections" />
    {/* Unmounted on the Updates tab, where UpdateControls owns the same text as
        its own live region, so a failure is announced once rather than twice. */}
    {showUpdatesNotice && <p id={updatesNoticeId} className="sr-only" role={state.updater.status === "failed" ? "alert" : "status"}>{updateDescription(state.updater)} Open the Updates tab to retry.</p>}
    <TabPanel id={active}>
      {active === "general" && <GeneralSection settings={settings} update={update} />}
      {active === "pointer" && <PointerSection settings={settings} update={update} />}
      {active === "cursor" && <CursorSection settings={settings} update={update} />}
      {active === "privacy" && <PrivacySection state={state} settings={settings} update={update} chooseTelemetry={chooseTelemetry} busy={busy} />}
      {active === "updates" && <UpdatesSection state={state} run={updateAction} cancel={cancelUpdate} sectionRef={updatesRef} />}
    </TabPanel>
  </div>;
}
