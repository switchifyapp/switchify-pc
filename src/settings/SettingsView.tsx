import { useEffect, useMemo, useRef, useState } from "react";
import { Settings } from "lucide-react";
import type { AppSettings, AppState } from "../types";
import { Tabs, TabPanel } from "./Tabs";
import { GeneralSection } from "./GeneralSection";
import { PointerSection } from "./PointerSection";
import { CursorSection } from "./CursorSection";
import { PrivacySection } from "./PrivacySection";
import { UpdatesSection, updateDescription, updateLiveness, type UpdateAction } from "./UpdatesSection";

const updatesNoticeId = "settings-updates-notice";

// Failed and cancelled are the updater states with something to act on that
// the global banner deliberately leaves out: the scheduled check fails for
// every offline user, so showing these app-wide would nag. They are surfaced
// within Settings instead, whose Updates panel is where their status lives.
type Standing = { text: string; status: "failed" | "cancelled" };
function standingOf(state: AppState): Standing | null {
  const { status } = state.updater;
  return status === "failed" || status === "cancelled" ? { text: updateDescription(state.updater), status } : null;
}

export function SettingsView({ state, settings, onChange, chooseTelemetry, updateAction, cancelUpdate, busy, focusUpdates, onUpdatesFocused }: { state: AppState; settings: AppSettings; onChange: (next: AppSettings) => void; chooseTelemetry: (enabled: boolean) => void; updateAction: (action: UpdateAction) => void; cancelUpdate: () => void; busy: boolean; focusUpdates: boolean; onUpdatesFocused: () => void }) {
  const updatesRef = useRef<HTMLElement>(null);
  // Opening straight to Updates starts there, rather than committing General
  // for one frame and announcing a failure for a tab already being opened.
  const [active, setActive] = useState(focusUpdates ? "updates" : "general");

  // The scheduled check flips a standing failure through "checking" and back
  // with the same text. Hold the last settled state through transient ones so
  // the marker does not blink and nothing is announced twice.
  const transient = state.updater.status === "checking" || state.updater.status === "applying";
  const settled = standingOf(state);
  const [standing, setStanding] = useState(settled);
  useEffect(() => { if (!transient) setStanding(settled); }, [transient, settled?.text, settled?.status]);

  // The off-tab notice speaks a failure the user has not been shown: once per
  // distinct failure, only while another tab is selected, never on tab
  // movement, and never for text already seen on the Updates panel, where
  // UpdateControls announces on entry as it always has. The region is always
  // mounted so it exists before any text arrives.
  const [notice, setNotice] = useState("");
  const seen = useRef(active === "updates" && standing ? standing.text : "");
  useEffect(() => {
    const text = standing?.text ?? "";
    if (active === "updates" || !text) { seen.current = text; setNotice(""); return; }
    if (text !== seen.current) {
      seen.current = text;
      setNotice(standing?.status === "failed" ? `${text} Open the Updates tab to retry.` : text);
    }
  }, [active, standing]);

  const tabs = useMemo(() => [
    { id: "general", label: "General" },
    { id: "pointer", label: "Pointer" },
    ...(state.capabilities.cursorOverlay ? [{ id: "cursor", label: "Cursor" }] : []),
    { id: "privacy", label: "Privacy" },
    // On the Updates tab the panel itself shows the reason, so no marker there.
    { id: "updates", label: "Updates", attention: standing && active !== "updates" ? standing.text : undefined },
  ], [state.capabilities.cursorOverlay, standing, active]);

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
    <p id={updatesNoticeId} className="sr-only" aria-live={standing ? updateLiveness(standing.status) : "polite"} aria-atomic="true">{notice}</p>
    <TabPanel id={active}>
      {active === "general" && <GeneralSection settings={settings} update={update} />}
      {active === "pointer" && <PointerSection settings={settings} update={update} />}
      {active === "cursor" && <CursorSection settings={settings} update={update} />}
      {active === "privacy" && <PrivacySection state={state} settings={settings} update={update} chooseTelemetry={chooseTelemetry} busy={busy} />}
      {active === "updates" && <UpdatesSection state={state} run={updateAction} cancel={cancelUpdate} sectionRef={updatesRef} />}
    </TabPanel>
  </div>;
}
