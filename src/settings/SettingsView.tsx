import { useEffect, useMemo, useRef, useState } from "react";
import { Settings } from "lucide-react";
import type { AppSettings, AppState } from "../types";
import { Tabs, TabPanel } from "./Tabs";
import { GeneralSection } from "./GeneralSection";
import { PointerSection } from "./PointerSection";
import { CursorSection } from "./CursorSection";
import { PrivacySection } from "./PrivacySection";
import { UpdatesSection, updateDescription, updateStatusRole, type UpdateAction } from "./UpdatesSection";

const updatesDescriptionId = "settings-updates-description";
const updatesNoticeId = "settings-updates-notice";

// Backend failure text is `context: error` with no terminal punctuation, so
// give it one before anything is appended.
const sentence = (text: string) => /[.!?]$/.test(text) ? text : `${text}.`;

export function SettingsView({ state, settings, onChange, chooseTelemetry, updateAction, cancelUpdate, busy, focusUpdates, onUpdatesFocused }: { state: AppState; settings: AppSettings; onChange: (next: AppSettings) => void; chooseTelemetry: (enabled: boolean) => void; updateAction: (action: UpdateAction) => void; cancelUpdate: () => void; busy: boolean; focusUpdates: boolean; onUpdatesFocused: () => void }) {
  const updatesRef = useRef<HTMLElement>(null);
  // Opening straight to Updates starts there, rather than committing General
  // for one frame and letting the off-tab notice fire for a tab that is
  // already being opened.
  const [active, setActive] = useState(focusUpdates ? "updates" : "general");
  const activeRef = useRef(active);
  useEffect(() => { activeRef.current = active; }, [active]);

  // Failed and cancelled are the only updater states with something to act on
  // that the global banner deliberately does not show. Their status line lives
  // in the Updates panel, which is unmounted on other tabs, so surface them
  // here: a marker on the tab with the reason as its description, and a live
  // region so the change is announced.
  const updaterNeedsAttention = state.updater.status === "failed" || state.updater.status === "cancelled";
  const updatesDescription = updaterNeedsAttention ? sentence(updateDescription(state.updater)) : "";

  // The live region only changes when the updater does, never on tab
  // movement, so an unchanged failure is spoken once. It stays silent when the
  // change lands on the Updates tab, where UpdateControls has its own region.
  const [updatesNotice, setUpdatesNotice] = useState("");
  useEffect(() => {
    setUpdatesNotice(updaterNeedsAttention && activeRef.current !== "updates" ? `${updatesDescription} Open the Updates tab to retry.` : "");
  }, [updaterNeedsAttention, updatesDescription]);
  // Arriving at Updates retires the notice: the panel now shows the same text,
  // and an emptied region has nothing to say if the user leaves again.
  useEffect(() => { if (active === "updates") setUpdatesNotice(""); }, [active]);

  const tabs = useMemo(() => [
    { id: "general", label: "General" },
    { id: "pointer", label: "Pointer" },
    ...(state.capabilities.cursorOverlay ? [{ id: "cursor", label: "Cursor" }] : []),
    { id: "privacy", label: "Privacy" },
    { id: "updates", label: "Updates", attention: updaterNeedsAttention, describedBy: updaterNeedsAttention ? updatesDescriptionId : undefined },
  ], [state.capabilities.cursorOverlay, updaterNeedsAttention]);

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
    {updaterNeedsAttention && <span id={updatesDescriptionId} className="sr-only">{updatesDescription}</span>}
    {/* Not rendered on Updates, so the panel's region is the only one there. */}
    {updaterNeedsAttention && active !== "updates" && <p id={updatesNoticeId} className="sr-only" role={updateStatusRole(state.updater)} aria-atomic="true">{updatesNotice}</p>}
    <TabPanel id={active}>
      {active === "general" && <GeneralSection settings={settings} update={update} />}
      {active === "pointer" && <PointerSection settings={settings} update={update} />}
      {active === "cursor" && <CursorSection settings={settings} update={update} />}
      {active === "privacy" && <PrivacySection state={state} settings={settings} update={update} chooseTelemetry={chooseTelemetry} busy={busy} />}
      {active === "updates" && <UpdatesSection state={state} run={updateAction} cancel={cancelUpdate} sectionRef={updatesRef} />}
    </TabPanel>
  </div>;
}
