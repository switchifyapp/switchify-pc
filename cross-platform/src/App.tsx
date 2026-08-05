import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  Accessibility, Bluetooth, CheckCircle2, ChevronRight, CircleHelp, Download,
  Home, Keyboard, MousePointer2, Plus, Power, Radio, RefreshCw, Save, Settings,
  ShieldCheck, SlidersHorizontal, Smartphone, Trash2, WifiOff, Wrench, X,
} from "lucide-react";
import { api } from "./api";
import type { AppSettings, AppState, SwitchProfile } from "./types";

type View = "home" | "devices" | "profiles" | "settings" | "support";

const bluetoothLabels: Record<AppState["bluetooth"], string> = {
  initializing: "Starting Bluetooth...", advertising: "Ready to connect", connected: "Device connected",
  poweredOff: "Bluetooth is off", unauthorized: "Bluetooth permission required",
  conflict: "Current Switchify PC is running", unsupported: "Bluetooth unavailable", error: "Bluetooth unavailable",
};

function NavButton({ active, icon, children, onClick }: { active: boolean; icon: ReactNode; children: ReactNode; onClick: () => void }) {
  return <button className="nav-button" data-active={active} onClick={onClick}>{icon}<span>{children}</span></button>;
}

function StatusIcon({ ok, children }: { ok: boolean; children: ReactNode }) {
  return <span className="status-icon" data-ok={ok}>{children}</span>;
}

function Toggle({ checked, disabled = false, label, onChange }: { checked: boolean; disabled?: boolean; label: string; onChange: (next: boolean) => void }) {
  return <label className="toggle-row" data-disabled={disabled}><span>{label}</span><input type="checkbox" checked={checked} disabled={disabled} onChange={(event) => onChange(event.target.checked)} /><span className="toggle" aria-hidden="true" /></label>;
}

function HomeView({ state, onDisconnect, onAccessibility, onSetup }: { state: AppState; onDisconnect: () => void; onAccessibility: () => void; onSetup: () => void }) {
  const bluetoothOk = state.bluetooth === "advertising" || state.bluetooth === "connected";
  return <div className="view">
    <header className="page-header"><div><h1>Switchify PC</h1><p>Android control for this computer</p></div><span className="preview-badge">Preview</span></header>
    <section className="connection-band" data-connected={state.bluetooth === "connected"}>
      <StatusIcon ok={bluetoothOk}>{bluetoothOk ? <Radio size={20} /> : <WifiOff size={20} />}</StatusIcon>
      <div><h2>{bluetoothLabels[state.bluetooth]}</h2><p>{state.connectedDeviceName ?? state.lastActivity?.message ?? "Waiting for a nearby Android device."}</p></div>
      {state.bluetooth === "connected" ? <button className="secondary" onClick={onDisconnect}><Power size={16} />Disconnect</button> : <button className="secondary" onClick={onSetup}><Wrench size={16} />Set up</button>}
    </section>
    <section className="status-list" aria-label="System status">
      <article><StatusIcon ok={bluetoothOk}><Bluetooth size={19} /></StatusIcon><div><h3>Bluetooth</h3><p>{bluetoothLabels[state.bluetooth]}</p></div></article>
      <article><StatusIcon ok={state.accessibility === "granted"}><Accessibility size={19} /></StatusIcon><div><h3>Input access</h3><p>{state.accessibility === "granted" ? "Ready" : "Permission required"}</p></div>{state.accessibility === "required" && <button className="text-button" onClick={onAccessibility}>Review</button>}</article>
      <article><StatusIcon ok><ShieldCheck size={19} /></StatusIcon><div><h3>Secure pairing</h3><p>{state.pairedDevices.length === 0 ? "No saved devices" : `${state.pairedDevices.length} saved device${state.pairedDevices.length === 1 ? "" : "s"}`}</p></div></article>
    </section>
    <section className="activity-panel" aria-live="polite"><span>Recent activity</span><p data-kind={state.lastActivity?.kind}>{state.lastActivity?.message ?? "No recent activity."}</p></section>
  </div>;
}

function DevicesView({ state, forget }: { state: AppState; forget: (id: string) => void }) {
  return <div className="view"><header className="page-header"><div><h1>Paired devices</h1><p>Android devices trusted by this preview</p></div><Smartphone size={24} /></header>
    {state.pairedDevices.length === 0 ? <div className="empty-state"><Smartphone size={28} /><h2>No paired devices</h2><p>Pair from Switchify Android while this preview is advertising.</p></div> :
      <div className="device-list">{state.pairedDevices.map((device) => <article key={device.deviceId}><Smartphone size={20} /><div><h2>{device.deviceName}</h2><p>{device.lastSeenAt ? `Last seen ${new Date(device.lastSeenAt).toLocaleString()}` : "Not connected yet"}</p></div><button className="icon-button danger-icon" title={`Forget ${device.deviceName}`} onClick={() => forget(device.deviceId)}><Trash2 size={18} /></button></article>)}</div>}
  </div>;
}

const newProfile = (): SwitchProfile => ({
  id: crypto.randomUUID(), version: 1, name: "New profile", provider: "mapped", builtIn: false,
  bindings: Array.from({ length: 8 }, (_, index) => ({ switchId: index + 1, type: "none" })),
});

function ProfileEditor({ profile, onClose, onSave, onDelete, busy }: { profile: SwitchProfile; onClose: () => void; onSave: (profile: SwitchProfile) => void; onDelete: (() => void) | null; busy: boolean }) {
  const [draft, setDraft] = useState(profile);
  const setBinding = (index: number, type: SwitchProfile["bindings"][number]["type"], value?: string, keys?: string[]) => {
    const defaults: Partial<Record<typeof type, string>> = { key: "Space", mouseButton: "left", mouseClick: "left", scroll: "down", media: "playPause" };
    const nextValue = value ?? defaults[type];
    const bindings = draft.bindings.map((binding, bindingIndex) => bindingIndex === index ? { switchId: index + 1, type, ...(nextValue ? { value: nextValue } : {}), ...(keys ? { keys } : {}), ...(type === "mouseClick" ? { clickCount: 1 } : {}) } : binding);
    setDraft({ ...draft, bindings });
  };
  return <div className="modal-backdrop"><section className="profile-dialog" role="dialog" aria-modal="true" aria-labelledby="profile-title">
    <header><div><h2 id="profile-title">{profile.builtIn ? profile.name : "Edit switch profile"}</h2><p>Map each physical switch to a desktop action.</p></div><button className="icon-button" title="Close" onClick={onClose}><X size={18} /></button></header>
    <label className="field"><span>Profile name</span><input value={draft.name} maxLength={50} disabled={profile.builtIn} onChange={(event) => setDraft({ ...draft, name: event.target.value })} /></label>
    <div className="binding-list">{draft.bindings.map((binding, index) => <div className="binding-row" key={binding.switchId}>
      <strong>Switch {binding.switchId}</strong>
      <select aria-label={`Switch ${binding.switchId} action`} value={binding.type} disabled={profile.builtIn} onChange={(event) => setBinding(index, event.target.value as typeof binding.type)}>
        <option value="none">No action</option><option value="key">Key</option><option value="shortcut">Shortcut</option><option value="mouseButton">Hold mouse button</option><option value="mouseClick">Mouse click</option><option value="scroll">Scroll</option><option value="media">Media</option>
      </select>
      {(binding.type === "key" || binding.type === "shortcut") && <input className="key-recorder" aria-label={`Switch ${binding.switchId} key`} placeholder="Select, then press key" value={binding.type === "shortcut" ? (binding.keys ?? []).join("+") : binding.value ?? ""} readOnly disabled={profile.builtIn} onKeyDown={(event) => { event.preventDefault(); const pressed = [event.ctrlKey && "Ctrl", event.altKey && "Alt", event.shiftKey && "Shift", event.metaKey && "Meta", event.key.length === 1 ? event.key.toUpperCase() : event.key].filter((key): key is string => Boolean(key)); const keys = [...new Set(pressed)]; setBinding(index, binding.type, binding.type === "key" ? keys.at(-1) : undefined, binding.type === "shortcut" ? keys : undefined); }} />}
      {(binding.type === "mouseButton" || binding.type === "mouseClick") && <select aria-label={`Switch ${binding.switchId} mouse button`} value={binding.value ?? "left"} disabled={profile.builtIn} onChange={(event) => setBinding(index, binding.type, event.target.value)}><option value="left">Left</option><option value="right">Right</option><option value="middle">Middle</option></select>}
      {binding.type === "scroll" && <select aria-label={`Switch ${binding.switchId} scroll direction`} value={binding.value ?? "down"} disabled={profile.builtIn} onChange={(event) => setBinding(index, binding.type, event.target.value)}><option value="up">Up</option><option value="down">Down</option><option value="left">Left</option><option value="right">Right</option></select>}
      {binding.type === "media" && <select aria-label={`Switch ${binding.switchId} media action`} value={binding.value ?? "playPause"} disabled={profile.builtIn} onChange={(event) => setBinding(index, binding.type, event.target.value)}><option value="playPause">Play / pause</option><option value="nextTrack">Next track</option><option value="previousTrack">Previous track</option><option value="volumeUp">Volume up</option><option value="volumeDown">Volume down</option><option value="mute">Mute</option></select>}
    </div>)}</div>
    <footer>{onDelete && <button className="secondary danger" disabled={busy} onClick={onDelete}><Trash2 size={16} />Delete</button>}<span /><button className="secondary" onClick={onClose}>Cancel</button>{!profile.builtIn && <button className="primary" disabled={busy || !draft.name.trim()} onClick={() => onSave({ ...draft, name: draft.name.trim() })}><Save size={16} />Save profile</button>}</footer>
  </section></div>;
}

function ProfilesView({ profiles, platform, saveProfile, deleteProfile, busy }: { profiles: SwitchProfile[]; platform: AppState["capabilities"]["platform"]; saveProfile: (profile: SwitchProfile) => void; deleteProfile: (id: string) => void; busy: boolean }) {
  const [editing, setEditing] = useState<SwitchProfile | null>(null);
  return <div className="view"><header className="page-header"><div><h1>Switch control</h1><p>Profiles available to physical switch sessions</p></div><button className="secondary" onClick={() => setEditing(newProfile())}><Plus size={16} />New profile</button></header>
    <div className="profile-list">{profiles.map((profile) => <button className="profile-row" key={profile.id} onClick={() => setEditing(profile)}><div className="profile-icon"><SlidersHorizontal size={19} /></div><div><h2>{profile.name}</h2><p>{profile.provider === "grid3" ? "Grid 3" : `${profile.bindings.filter((binding) => binding.type !== "none").length} mapped switches`}</p></div><span>{profile.builtIn ? "Built in" : "Custom"}</span><ChevronRight size={18} /></button>)}</div>
    {platform === "macos" && <p className="capability-note">Grid 3 profiles are available on Windows only.</p>}
    {editing && <ProfileEditor profile={editing} busy={busy} onClose={() => setEditing(null)} onSave={(profile) => { saveProfile(profile); setEditing(null); }} onDelete={editing.builtIn ? null : () => { deleteProfile(editing.id); setEditing(null); }} />}
  </div>;
}

function SettingGroup({ title, description, children }: { title: string; description: string; children: ReactNode }) {
  return <section className="setting-group"><header><h2>{title}</h2><p>{description}</p></header><div className="setting-controls">{children}</div></section>;
}

const pointerSpeedOptions = [5, 25, 50, 75, 100] as const;
const pointerSpeedValues = Array.from({ length: 45 }, (_, index) => (index + 1) * 5);
const repeatIntervalOptions = [100, 250, 500, 1000] as const;
const accelerationOptions = [
  { value: 0, label: "Off" },
  { value: 500, label: "Short" },
  { value: 1000, label: "Medium" },
  { value: 2000, label: "Long" },
] as const;

function movementValue(base: number, scale: number) {
  const value = Math.min(50, Math.max(1, Math.round((base * scale / 100) * 2) / 2));
  return Number.isInteger(value) ? String(value) : value.toFixed(1);
}

function SettingsView({ state, settings, setSettings, save, checkUpdates, busy }: { state: AppState; settings: AppSettings; setSettings: (next: AppSettings) => void; save: () => void; checkUpdates: () => void; busy: boolean }) {
  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => setSettings({ ...settings, [key]: value });
  return <div className="view"><header className="page-header"><div><h1>Settings</h1><p>Startup, pointer, privacy, and updates</p></div><Settings size={24} /></header>
    <SettingGroup title="General" description="System startup and background behavior."><Toggle label="Start with system" checked={settings.startWithSystem} onChange={(value) => update("startWithSystem", value)} /></SettingGroup>
    <SettingGroup title="Pointer" description="Movement and visual feedback.">
      <fieldset className="pointer-speed"><legend>Pointer speed <strong>{settings.pointerScalePercent}%</strong></legend><div className="segmented compact five">
        {pointerSpeedOptions.map((value) => <button type="button" key={value} aria-label={`${value}% pointer speed`} aria-pressed={settings.pointerScalePercent === value} onClick={() => update("pointerScalePercent", value)}>{value}%</button>)}
      </div><label className="exact-speed"><span>Exact speed</span><select aria-label="Exact pointer speed" value={settings.pointerScalePercent} onChange={(event) => update("pointerScalePercent", Number(event.target.value))}>
        {pointerSpeedValues.map((value) => <option key={value} value={value}>{value}%</option>)}
      </select></label><div className="movement-values" aria-label="Pointer movement values">
        {([{"label":"Small","base":4.5},{"label":"Medium","base":12},{"label":"Large","base":26}] as const).map(({ label, base }) => <div key={label}><span>{label}</span><strong>{movementValue(base, settings.pointerScalePercent)}</strong></div>)}
      </div></fieldset>
      <div className="repeat-settings">
        <Toggle label="Repeat mouse movement" checked={settings.mouseRepeatEnabled} onChange={(value) => update("mouseRepeatEnabled", value)} />
        <div className="repeat-options" data-disabled={!settings.mouseRepeatEnabled}>
          <fieldset disabled={!settings.mouseRepeatEnabled}><legend>Movement interval</legend><div className="segmented compact four">
            {repeatIntervalOptions.map((value) => <button type="button" key={value} aria-pressed={settings.moveRepeatIntervalMs === value} onClick={() => update("moveRepeatIntervalMs", value)}>{value / 1000}s</button>)}
          </div></fieldset>
          <fieldset disabled={!settings.mouseRepeatEnabled}><legend>Movement acceleration</legend><div className="segmented compact four">
            {accelerationOptions.map(({ value, label }) => <button type="button" key={value} aria-pressed={settings.mouseRepeatAccelerationDurationMs === value} onClick={() => update("mouseRepeatAccelerationDurationMs", value)}>{label}</button>)}
          </div></fieldset>
          <fieldset disabled={!settings.mouseRepeatEnabled}><legend>Scroll interval</legend><div className="segmented compact four">
            {repeatIntervalOptions.map((value) => <button type="button" key={value} aria-pressed={settings.scrollRepeatIntervalMs === value} onClick={() => update("scrollRepeatIntervalMs", value)}>{value / 1000}s</button>)}
          </div></fieldset>
        </div>
      </div>
      {state.capabilities.cursorOverlay && <>
        <Toggle label="Show cursor overlay" checked={settings.cursorOverlayEnabled} onChange={(value) => update("cursorOverlayEnabled", value)} />
        <div className="overlay-options" data-disabled={!settings.cursorOverlayEnabled}>
          <fieldset disabled={!settings.cursorOverlayEnabled}><legend>Overlay visibility</legend><div className="segmented compact">
            {(["onInput", "whileControlling"] as const).map((value) => <button type="button" key={value} aria-pressed={settings.cursorOverlayVisibility === value} onClick={() => update("cursorOverlayVisibility", value)}>{value === "onInput" ? "On input" : "While controlling"}</button>)}
          </div><p className="setting-note">On input hides shortly after pointer activity stops. While controlling stays visible until the session ends.</p></fieldset>
          <fieldset disabled={!settings.cursorOverlayEnabled}><legend>Overlay size</legend><div className="segmented compact three">
            {(["small", "medium", "large"] as const).map((value) => <button type="button" key={value} aria-pressed={settings.cursorOverlaySize === value} onClick={() => update("cursorOverlaySize", value)}>{value[0].toUpperCase() + value.slice(1)}</button>)}
          </div></fieldset>
          <fieldset disabled={!settings.cursorOverlayEnabled}><legend>Overlay color</legend><div className="color-options">
            {(["red", "green", "blue", "yellow", "white"] as const).map((value) => <label key={value} title={value[0].toUpperCase() + value.slice(1)}><input type="radio" name="overlay-color" value={value} checked={settings.cursorOverlayColor === value} onChange={() => update("cursorOverlayColor", value)} /><span className={`color-swatch ${value}`} /><span className="sr-only">{value[0].toUpperCase() + value.slice(1)}</span></label>)}
          </div></fieldset>
          <Toggle label="Show crosshairs" disabled={!settings.cursorOverlayEnabled} checked={settings.cursorCrosshairs} onChange={(value) => update("cursorCrosshairs", value)} />
        </div>
      </>}
    </SettingGroup>
    <SettingGroup title="Privacy" description="Sanitized application health reports only."><Toggle label="Share diagnostic data" checked={settings.shareDiagnostics} onChange={(value) => update("shareDiagnostics", value)} /></SettingGroup>
    <SettingGroup title="Updates" description={`Switchify PC Preview ${state.version}`}><button className="secondary" onClick={checkUpdates} disabled={busy}><RefreshCw size={16} />Check for updates</button></SettingGroup>
    <div className="save-bar"><button className="primary" onClick={save} disabled={busy}>{busy ? "Saving..." : "Save settings"}</button></div>
  </div>;
}

function SupportView({ state, busy, perform }: { state: AppState; busy: boolean; perform: (operation: () => Promise<AppState>) => void }) {
  const [tab, setTab] = useState<"setup" | "troubleshooting">("setup");
  const bluetoothReady = state.bluetooth === "advertising" || state.bluetooth === "connected";
  return <div className="view"><header className="page-header"><div><h1>Support</h1><p>Connection setup and system diagnostics</p></div><CircleHelp size={24} /></header>
    <div className="segmented" role="tablist" aria-label="Support view"><button role="tab" aria-selected={tab === "setup"} onClick={() => setTab("setup")}>Setup</button><button role="tab" aria-selected={tab === "troubleshooting"} onClick={() => setTab("troubleshooting")}>Troubleshooting</button></div>
    {tab === "setup" ? <section className="task-list" aria-label="Setup status">
      <article><StatusIcon ok={bluetoothReady}>{bluetoothReady ? <CheckCircle2 size={19} /> : <Bluetooth size={19} />}</StatusIcon><div><h2>Bluetooth</h2><p>{bluetoothLabels[state.bluetooth]}</p></div></article>
      <article><StatusIcon ok={state.accessibility === "granted"}><Accessibility size={19} /></StatusIcon><div><h2>Input access</h2><p>{state.accessibility === "granted" ? "Ready" : "Required for keyboard and pointer control"}</p></div>{state.accessibility === "required" && <button className="secondary" disabled={busy} onClick={() => perform(() => api.checkAccessibility(true))}>Review access</button>}</article>
      <article><StatusIcon ok={state.pairedDevices.length > 0}><Smartphone size={19} /></StatusIcon><div><h2>Android device</h2><p>{state.pairedDevices.length > 0 ? `${state.pairedDevices.length} paired` : "Open Switchify on Android and select this computer"}</p></div></article>
      <article><StatusIcon ok={state.bluetooth === "connected"}><Radio size={19} /></StatusIcon><div><h2>Connection</h2><p>{state.connectedDeviceName ?? "Waiting for a paired device"}</p></div></article>
    </section> : <section className="task-list" aria-label="Troubleshooting actions">
      <article><Bluetooth size={20} /><div><h2>Bluetooth connection</h2><p>{bluetoothLabels[state.bluetooth]}</p></div><button className="secondary" disabled={busy} onClick={() => perform(api.disconnectAll)}><Power size={16} />Disconnect</button></article>
      <article><Accessibility size={20} /><div><h2>Input access</h2><p>{state.accessibility === "granted" ? "Permission is available" : "Permission needs attention"}</p></div><button className="secondary" disabled={busy} onClick={() => perform(() => api.checkAccessibility(true))}><RefreshCw size={16} />Check</button></article>
      <article><RefreshCw size={20} /><div><h2>Application update</h2><p>Preview {state.version}</p></div><button className="secondary" disabled={busy} onClick={() => perform(api.checkForUpdates)}>Check</button></article>
      <article><Download size={20} /><div><h2>Diagnostics</h2><p>Export sanitized health and capability data</p></div><button className="secondary" disabled={busy} onClick={() => perform(api.exportDiagnostics)}><Download size={16} />Export</button></article>
    </section>}
    {state.lastActivity && <section className="activity-panel" aria-live="polite"><span>Recent activity</span><p data-kind={state.lastActivity.kind}>{state.lastActivity.message}</p></section>}
  </div>;
}

export function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [view, setView] = useState<View>("home");
  const [profiles, setProfiles] = useState<SwitchProfile[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const settingsDirty = useRef(false);

  const syncState = (next: AppState) => {
    setState(next);
    if (!settingsDirty.current) setSettings(next.settings);
  };

  const perform = async (operation: () => Promise<AppState>) => {
    setBusy(true); setError(null);
    try { syncState(await operation()); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  };

  const saveSettings = async () => {
    if (!settings) return;
    setBusy(true); setError(null);
    try {
      const next = await api.saveSettings(settings);
      settingsDirty.current = false;
      setState(next);
      setSettings(next.settings);
    } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  };

  useEffect(() => {
    let unlisten: () => void = () => {};
    void api.state().then(syncState).catch((reason) => setError(String(reason)));
    void api.onState(syncState).then((stop) => { unlisten = stop; });
    return () => unlisten();
  }, []);

  useEffect(() => { if (view === "profiles") void api.listProfiles().then(setProfiles).catch((reason) => setError(String(reason))); }, [view]);
  const nav = useMemo(() => [
    ["home", "Home", <Home size={19} />], ["devices", "Devices", <Smartphone size={19} />],
    ["profiles", "Switch control", <Keyboard size={19} />], ["settings", "Settings", <Settings size={19} />],
    ["support", "Support", <CircleHelp size={19} />],
  ] as const, []);

  if (!state || !settings) return <div className="loading"><RefreshCw className="spin" size={24} /><span>Starting Switchify PC Preview...</span></div>;
  return <div className="app-shell">
    <aside><div className="brand"><span className="brand-mark"><MousePointer2 size={21} /></span><div><strong>Switchify</strong><small>PC Preview</small></div></div><nav>{nav.map(([id, label, icon]) => <NavButton key={id} active={view === id} icon={icon} onClick={() => setView(id)}>{label}</NavButton>)}</nav><div className="sidebar-footer"><CircleHelp size={16} /><span>{state.capabilities.platform === "macos" ? "macOS" : "Windows"} preview</span></div></aside>
    <main>
      {error && <div className="error-banner" role="alert">{error}<button onClick={() => setError(null)}>Dismiss</button></div>}
      {view === "home" && <HomeView state={state} onDisconnect={() => void perform(api.disconnectAll)} onAccessibility={() => void perform(() => api.checkAccessibility(true))} onSetup={() => setView("support")} />}
      {view === "devices" && <DevicesView state={state} forget={(id) => void perform(() => api.forgetDevice(id))} />}
      {view === "profiles" && <ProfilesView profiles={profiles} platform={state.capabilities.platform} busy={busy} saveProfile={(profile) => { setBusy(true); setError(null); void api.saveProfile(profile).then(setProfiles).catch((reason) => setError(String(reason))).finally(() => setBusy(false)); }} deleteProfile={(id) => { setBusy(true); setError(null); void api.deleteProfile(id).then(setProfiles).catch((reason) => setError(String(reason))).finally(() => setBusy(false)); }} />}
      {view === "settings" && <SettingsView state={state} settings={settings} setSettings={(next) => { settingsDirty.current = true; setSettings(next); }} save={() => void saveSettings()} checkUpdates={() => void perform(api.checkForUpdates)} busy={busy} />}
      {view === "support" && <SupportView state={state} busy={busy} perform={(operation) => void perform(operation)} />}
    </main>
    {state.pendingPairing && <div className="modal-backdrop"><section className="pairing-dialog" role="dialog" aria-modal="true" aria-labelledby="pairing-title"><Smartphone size={26} /><h2 id="pairing-title">Pair {state.pendingPairing.deviceName}</h2><p>Confirm that this code matches Switchify Android.</p><output>{state.pendingPairing.verificationCode}</output><div><button className="secondary danger" disabled={busy} onClick={() => void perform(() => api.rejectPairing(state.pendingPairing!.requestId))}>Reject</button><button className="primary" disabled={busy} onClick={() => void perform(() => api.approvePairing(state.pendingPairing!.requestId))}>Accept</button></div></section></div>}
  </div>;
}
