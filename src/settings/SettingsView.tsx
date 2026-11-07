import { useEffect, useLayoutEffect, useRef } from "react";
import type { AppSettings, AppState, UpdateState } from "../types";
import { GeneralSection } from "./GeneralSection";
import { InputSection } from "./InputSection";
import { PrivacySection } from "./PrivacySection";
import { UpdatesSection, type UpdateAction } from "./UpdatesSection";
import { LicenceSection } from "./LicenceSection";
import { AboutSection } from "./AboutSection";
import { AccountSection } from "./AccountSection";
import { PageHead } from "../find/Find";
import { Button } from "../ui/controls";
import type { FindContext } from "../find/index";

const categories = [
  { id: "general", title: "General", description: "Starting Switchify with your computer." },
  { id: "input", title: "Input", description: "Dwell clicking and repeating held keys." },
  { id: "privacy", title: "Privacy", description: "Choose whether to share anonymous diagnostics." },
  { id: "updates", title: "Updates", description: "Check, download and install app updates." },
  { id: "account", title: "Account and licence", description: "Sign in, sync settings and manage this computer's licence." },
  { id: "about", title: "About", description: "App version, source code and credits." },
];

export function SettingsView({ section, onSectionChange, arriving, state, updater, settings, onChange, chooseTelemetry, updateAction, cancelUpdate, busy, focusUpdates, onUpdatesFocused, onUpdatesShown, ctx }: { section: string | null; onSectionChange: (section: string | null) => void; arriving: boolean; state: AppState; updater: UpdateState; settings: AppSettings; onChange: (next: AppSettings) => void; chooseTelemetry: (enabled: boolean) => void; updateAction: (action: UpdateAction) => void; cancelUpdate: () => void; busy: boolean; focusUpdates: boolean; onUpdatesFocused: () => void; onUpdatesShown: (shown: boolean) => void; ctx: FindContext }) {
  const updatesRef = useRef<HTMLElement>(null);
  const root = useRef<HTMLDivElement>(null);
  const previous = useRef<string | null | undefined>(undefined);
  const category = section === "licence" ? "account" : section;
  const showAccount = ctx.account?.available !== false;
  const updatesVisible = category === "updates";

  useEffect(() => {
    onUpdatesShown(updatesVisible);
    return () => onUpdatesShown(false);
  }, [onUpdatesShown, updatesVisible]);

  useLayoutEffect(() => {
    // Mounting after Find is cleared must leave focus in Find. Only category
    // changes move focus; Find arrivals and update notices own their focus.
    if (previous.current !== undefined && !arriving && !focusUpdates) {
      const target = category ? root.current?.querySelector<HTMLElement>("h2") : previous.current ? root.current?.querySelector<HTMLElement>(`[data-category="${previous.current}"]`) : null;
      if (target) { target.tabIndex = category ? -1 : 0; target.focus(); target.scrollIntoView?.({ block: "nearest" }); }
    }
    previous.current = category;
  }, [category]);

  useEffect(() => {
    if (!focusUpdates) return;
    updatesRef.current?.scrollIntoView?.({ block: "start" });
    updatesRef.current?.focus({ preventScroll: true });
    onUpdatesFocused();
  }, [focusUpdates, onUpdatesFocused]);

  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => onChange({ ...settings, [key]: value });

  return <div ref={root} className="view settings-view">
    <PageHead title="Settings" blurb="How Switchify PC behaves on this computer, and your account." />
    {!category ? <>
      <nav className="grid settings-categories" aria-label="Settings categories">
        {categories.map((item) => <Button key={item.id} className="tile tile-jump" data-category={item.id} aria-label={item.id === "account" && !showAccount ? "Licence" : item.title} onClick={() => onSectionChange(item.id)}>
          <span className="tile-title">{item.id === "account" && !showAccount ? "Licence" : item.title}</span>
          <span className="tile-note">{item.id === "account" && !showAccount ? "Manage this computer's licence." : item.description}</span>
        </Button>)}
      </nav>
    </> : <>
    <div><Button className="secondary" onClick={() => onSectionChange(null)}>Back to settings</Button></div>
    {category === "general" && <GeneralSection settings={settings} update={update} />}
    {category === "input" && <InputSection settings={settings} update={update} />}
    {category === "privacy" && <PrivacySection state={state} settings={settings} update={update} chooseTelemetry={chooseTelemetry} busy={busy} />}
    {category === "updates" && <UpdatesSection state={state} update={updater} run={updateAction} cancel={cancelUpdate} sectionRef={updatesRef} />}
    {category === "about" && <AboutSection state={state} />}
    {category === "account" && <>
    {showAccount && <section className="setting-group" data-section="account" data-setting="account" aria-labelledby="settings-account-heading">
      <header><h2 id="settings-account-heading">Account</h2><p>Bring your settings to your other computers.</p></header>
      <div className="setting-controls"><AccountSection settings={settings} onSettingsChange={onChange} /></div>
    </section>}
    <LicenceSection state={state} />
    </>}
    </>}
  </div>;
}
