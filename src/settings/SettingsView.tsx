import { useEffect, useRef } from "react";
import type { AppSettings, AppState, UpdateState } from "../types";
import { GeneralSection } from "./GeneralSection";
import { InputSection } from "./InputSection";
import { PrivacySection } from "./PrivacySection";
import { UpdatesSection, type UpdateAction } from "./UpdatesSection";
import { LicenceSection } from "./LicenceSection";
import { AboutSection } from "./AboutSection";
import { AccountSection } from "./AccountSection";
import { AlsoSee, PageHead, SectionStrip, type Jump } from "../find/Find";
import type { FindContext } from "../find/index";

// One page: every section in turn, with the strip at the top showing what
// each is set to. The strip, Find and the update banner all jump to a
// section rather than switching a tab.
export function SettingsView({ state, updater, settings, onChange, chooseTelemetry, updateAction, cancelUpdate, busy, focusUpdates, onUpdatesFocused, onUpdatesShown, ctx, jump }: { state: AppState; updater: UpdateState; settings: AppSettings; onChange: (next: AppSettings) => void; chooseTelemetry: (enabled: boolean) => void; updateAction: (action: UpdateAction) => void; cancelUpdate: () => void; busy: boolean; focusUpdates: boolean; onUpdatesFocused: () => void; onUpdatesShown: (shown: boolean) => void; ctx: FindContext; jump: Jump }) {
  const updatesRef = useRef<HTMLElement>(null);

  // App owns the standing update failure and what has been said about it; the
  // Updates panel, which shows it, is on screen whenever this page is.
  useEffect(() => {
    onUpdatesShown(true);
    return () => onUpdatesShown(false);
  }, [onUpdatesShown]);

  useEffect(() => {
    if (!focusUpdates) return;
    updatesRef.current?.scrollIntoView?.({ block: "start" });
    updatesRef.current?.focus({ preventScroll: true });
    onUpdatesFocused();
  }, [focusUpdates, onUpdatesFocused]);

  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => onChange({ ...settings, [key]: value });

  return <div className="view settings-view">
    <PageHead title="Settings" blurb="How Switchify PC behaves on this computer, and your account." />
    <SectionStrip page="settings" ctx={ctx} jump={jump} />
    <GeneralSection settings={settings} update={update} />
    <InputSection settings={settings} update={update} />
    <PrivacySection state={state} settings={settings} update={update} chooseTelemetry={chooseTelemetry} busy={busy} />
    <UpdatesSection state={state} update={updater} run={updateAction} cancel={cancelUpdate} sectionRef={updatesRef} />
    <LicenceSection state={state} />
    <AboutSection state={state} />
    {ctx.account?.available !== false && <section className="setting-group" data-section="account" data-setting="account" aria-labelledby="settings-account-heading">
      <header><h2 id="settings-account-heading">Account</h2><p>Bring your settings to your other computers.</p></header>
      <div className="setting-controls"><AccountSection settings={settings} onSettingsChange={onChange} /></div>
    </section>}
    <AlsoSee page="settings" ctx={ctx} jump={jump} />
  </div>;
}
