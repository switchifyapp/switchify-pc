import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  Accessibility, Bluetooth, CheckCircle2, ChevronRight, CircleHelp, Download,
  Copy, Home, Keyboard, Plus, Power, Radio, RefreshCw, Save, Settings,
  ShieldCheck, SlidersHorizontal, Smartphone, Trash2, WifiOff, Wrench, X,
} from "lucide-react";
import { api, type ProfileExitAction } from "./api";
import type { AppSettings, AppState, PendingPairing, SwitchProfile, UpdateState } from "./types";

type View = "home" | "devices" | "profiles" | "settings" | "support";

const brandIconUrl = new URL("../src-tauri/icons/icon.png", import.meta.url).href;
const androidQrUrl = new URL("./assets/android-download-qr.png", import.meta.url).href;
const androidDownloadUrl = "https://play.google.com/store/apps/details?id=com.enaboapps.switchify";

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

function AccessibilityCopy({ state, detailed = false }: { state: AppState; detailed?: boolean }) {
  if (state.accessibility === "granted") return <p>Ready</p>;
  if (state.accessibility === "unavailable") return <p>Unavailable on this system</p>;
  if (!detailed || state.capabilities.platform !== "macos") return <p>Permission required</p>;
  return <>
    <p>Enable “Switchify PC” in Accessibility, then return to the app. Status updates automatically.</p>
    <p className="permission-recovery">If it is already enabled but access is still required, select the stale row, click Remove, return to Switchify, reopen Accessibility Settings, and enable the newly added entry.</p>
  </>;
}

function HomeView({ state, onDisconnect, onAccessibility, onSetup }: { state: AppState; onDisconnect: () => void; onAccessibility: () => void; onSetup: () => void }) {
  const bluetoothOk = state.bluetooth === "advertising" || state.bluetooth === "connected";
  return <div className="view">
    <header className="page-header"><div><h1>Switchify PC</h1><p>Android control for this computer</p></div></header>
    <section className="connection-band" data-connected={state.bluetooth === "connected"}>
      <StatusIcon ok={bluetoothOk}>{bluetoothOk ? <Radio size={20} /> : <WifiOff size={20} />}</StatusIcon>
      <div><h2>{bluetoothLabels[state.bluetooth]}</h2><p>{state.connectedDeviceName ?? state.lastActivity?.message ?? "Waiting for a nearby Android device."}</p></div>
      {state.bluetooth === "connected" ? <button className="secondary" onClick={onDisconnect}><Power size={16} />Disconnect</button> : <button className="secondary" onClick={onSetup}><Wrench size={16} />Set up</button>}
    </section>
    <section className="status-list" aria-label="System status">
      <article><StatusIcon ok={bluetoothOk}><Bluetooth size={19} /></StatusIcon><div><h3>Bluetooth</h3><p>{bluetoothLabels[state.bluetooth]}</p></div></article>
      <article><StatusIcon ok={state.accessibility === "granted"}><Accessibility size={19} /></StatusIcon><div><h3>Input access</h3><AccessibilityCopy state={state} /></div>{state.accessibility === "required" && <button className="text-button" onClick={onAccessibility}>Open Accessibility Settings</button>}</article>
      <article><StatusIcon ok><ShieldCheck size={19} /></StatusIcon><div><h3>Secure pairing</h3><p>{state.pairedDevices.length === 0 ? "No saved devices" : `${state.pairedDevices.length} saved device${state.pairedDevices.length === 1 ? "" : "s"}`}</p></div></article>
    </section>
    <section className="activity-panel" aria-live="polite"><span>Recent activity</span><p data-kind={state.lastActivity?.kind}>{state.lastActivity?.message ?? "No recent activity."}</p></section>
  </div>;
}

function DevicesView({ state, forget }: { state: AppState; forget: (id: string) => void }) {
  return <div className="view"><header className="page-header"><div><h1>Paired devices</h1><p>Android devices trusted by this computer</p></div><Smartphone size={24} /></header>
    {state.pairedDevices.length === 0 ? <div className="empty-state"><Smartphone size={28} /><h2>No paired devices</h2><p>Pair from Switchify Android while this computer is advertising.</p></div> :
      <div className="device-list">{state.pairedDevices.map((device) => <article key={device.deviceId}><Smartphone size={20} /><div><h2>{device.deviceName}</h2><p>{device.lastSeenAt ? `Last seen ${new Date(device.lastSeenAt).toLocaleString()}` : "Not connected yet"}</p></div><button className="icon-button danger-icon" title={`Forget ${device.deviceName}`} onClick={() => forget(device.deviceId)}><Trash2 size={18} /></button></article>)}</div>}
  </div>;
}

const newProfile = (): SwitchProfile => ({
  id: crypto.randomUUID(), version: 1, name: "New profile", provider: "mapped", builtIn: false,
  bindings: Array.from({ length: 8 }, (_, index) => ({ switchId: index + 1, type: "none" })),
});

const modifierKeys = ["Ctrl", "Alt", "Shift", "Meta"];
const namedKeys = new Set(["Space", "Enter", "Escape", "Tab", "Backspace", "Delete", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", "Home", "End", "PageUp", "PageDown", ...modifierKeys, ...Array.from({ length: 12 }, (_, index) => `F${index + 1}`)]);
const friendlyKeys: Record<string, string> = { ArrowUp: "Up Arrow", ArrowDown: "Down Arrow", ArrowLeft: "Left Arrow", ArrowRight: "Right Arrow", Meta: "Command / Windows" };

function canonicalKey(key: string) {
  const aliases: Record<string, string> = { " ": "Space", Esc: "Escape", Control: "Ctrl", OS: "Meta" };
  const canonical = aliases[key] ?? key;
  return canonical.length === 1 ? canonical.toUpperCase() : canonical;
}

function friendlyKey(key: string) {
  return friendlyKeys[key] ?? key;
}

function bindingLabel(binding: SwitchProfile["bindings"][number]) {
  return binding.type === "shortcut"
    ? (binding.keys ?? []).map(friendlyKey).join(" + ")
    : friendlyKey(binding.value ?? "");
}

function isValidKey(key: string) {
  return namedKeys.has(key) || /^[A-Z0-9]$/.test(key);
}

function bindingSignature(binding: SwitchProfile["bindings"][number]) {
  if (binding.type === "shortcut") return JSON.stringify([binding.type, [...(binding.keys ?? [])].sort()]);
  if (binding.type === "mouseClick") return JSON.stringify([binding.type, binding.value, binding.clickCount ?? 1]);
  return JSON.stringify([binding.type, binding.value ?? null]);
}

function validateProfileDraft(draft: SwitchProfile, profiles: SwitchProfile[]) {
  const errors: Record<string, string> = {};
  const name = draft.name.trim();
  if (!name) errors.name = "Enter a profile name.";
  else if (draft.name.length > 50) errors.name = "Use 50 characters or fewer.";
  else if (profiles.some((candidate) => candidate.id !== draft.id && candidate.name.trim().toLocaleLowerCase() === name.toLocaleLowerCase())) errors.name = "Profile names must be unique.";

  const signatures = new Map<string, number>();
  for (const binding of draft.bindings) {
    const key = `binding-${binding.switchId}`;
    if (binding.type === "key" && !isValidKey(binding.value ?? "")) errors[key] = "Record a valid key.";
    if (binding.type === "shortcut") {
      const keys = binding.keys ?? [];
      if (keys.length === 0) errors[key] = "Record a shortcut.";
      else if (keys.length > 4 || keys.some((item) => !isValidKey(item)) || new Set(keys).size !== keys.length) errors[key] = "Record a valid shortcut of up to four different keys.";
      else if (keys.every((item) => modifierKeys.includes(item))) errors[key] = "Include a non-modifier key.";
    }
    if (binding.type === "none" || errors[key]) continue;
    const signature = bindingSignature(binding);
    const firstSwitch = signatures.get(signature);
    if (firstSwitch) {
      errors[key] = `This duplicates Switch ${firstSwitch}.`;
      errors[`binding-${firstSwitch}`] = `This duplicates Switch ${binding.switchId}.`;
    } else signatures.set(signature, binding.switchId);
  }
  return errors;
}

function duplicateProfile(source: SwitchProfile, profiles: SwitchProfile[]): SwitchProfile {
  const baseName = `${source.name} copy`;
  let name = baseName;
  for (let suffix = 2; profiles.some((profile) => profile.name.trim().toLocaleLowerCase() === name.toLocaleLowerCase()); suffix += 1) name = `${baseName} ${suffix}`;
  return {
    ...structuredClone(source),
    id: crypto.randomUUID(),
    version: 1,
    name,
    provider: "mapped",
    builtIn: false,
  };
}

type ProfileEditorProps = {
  profile: SwitchProfile;
  profiles: SwitchProfile[];
  onClose: () => void;
  onSave: (profile: SwitchProfile) => Promise<void>;
  onDelete: (() => Promise<void>) | null;
  onDuplicate: () => void;
  onDirtyChange: (dirty: boolean) => void;
  nativeExitRequest: ProfileExitAction | null;
  onConfirmNativeExit: () => void;
  onCancelNativeExit: () => void;
  busy: boolean;
};

function ProfileEditor({ profile, profiles, onClose, onSave, onDelete, onDuplicate, onDirtyChange, nativeExitRequest, onConfirmNativeExit, onCancelNativeExit, busy }: ProfileEditorProps) {
  const [draft, setDraft] = useState(profile);
  const [confirmation, setConfirmation] = useState<"discard" | "delete" | "duplicate" | "native" | null>(null);
  const [operationError, setOperationError] = useState<string | null>(null);
  const dialogRef = useRef<HTMLElement>(null);
  const confirmationButtonRef = useRef<HTMLButtonElement>(null);
  const dirty = JSON.stringify(draft) !== JSON.stringify(profile);
  const errors = validateProfileDraft(draft, profiles);
  const firstError = Object.keys(errors)[0];

  useEffect(() => {
    onDirtyChange(dirty);
    return () => onDirtyChange(false);
  }, [dirty, onDirtyChange]);

  useEffect(() => {
    if (confirmation) confirmationButtonRef.current?.focus();
    else dialogRef.current?.focus();
  }, [confirmation]);

  useEffect(() => {
    if (nativeExitRequest) setConfirmation("native");
  }, [nativeExitRequest]);

  const setBinding = (index: number, type: SwitchProfile["bindings"][number]["type"], value?: string, keys?: string[]) => {
    const defaults: Partial<Record<typeof type, string>> = { key: "Space", mouseButton: "left", mouseClick: "left", scroll: "down", media: "playPause" };
    const nextValue = value ?? defaults[type];
    const bindings = draft.bindings.map((binding, bindingIndex) => bindingIndex === index ? { switchId: index + 1, type, ...(nextValue ? { value: nextValue } : {}), ...(keys ? { keys } : {}), ...(type === "mouseClick" ? { clickCount: 1 } : {}) } : binding);
    setDraft({ ...draft, bindings });
  };
  const requestClose = () => dirty ? setConfirmation("discard") : onClose();
  const runSave = async () => {
    if (firstError) {
      dialogRef.current?.querySelector<HTMLElement>(`[data-error-key="${firstError}"]`)?.focus();
      return;
    }
    setOperationError(null);
    try { await onSave({ ...draft, name: draft.name.trim() }); }
    catch (reason) { setOperationError(String(reason)); dialogRef.current?.focus(); }
  };
  const runDelete = async () => {
    if (!onDelete) return;
    setOperationError(null);
    try { await onDelete(); }
    catch (reason) { setConfirmation(null); setOperationError(String(reason)); dialogRef.current?.focus(); }
  };
  const recordKey = (index: number, binding: SwitchProfile["bindings"][number], event: React.KeyboardEvent<HTMLInputElement>) => {
    event.preventDefault();
    const pressed = canonicalKey(event.key);
    const modifiers = [event.ctrlKey && "Ctrl", event.altKey && "Alt", event.shiftKey && "Shift", event.metaKey && "Meta"].filter((key): key is string => Boolean(key));
    const keys = [...new Set([...modifiers, pressed])];
    setBinding(index, binding.type, binding.type === "key" ? pressed : undefined, binding.type === "shortcut" ? keys : undefined);
  };
  const trapFocus = (event: React.KeyboardEvent<HTMLElement>) => {
    if (event.key === "Escape") { event.preventDefault(); if (confirmation === "native") onCancelNativeExit(); confirmation ? setConfirmation(null) : requestClose(); return; }
    if (event.key !== "Tab") return;
    const controls = [...(dialogRef.current?.querySelectorAll<HTMLElement>('button:not(:disabled), input:not(:disabled), select:not(:disabled)') ?? [])].filter((control) => control.offsetParent !== null || control === document.activeElement);
    if (controls.length === 0) return;
    const first = controls[0]; const last = controls.at(-1)!;
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
  };

  const confirmationTitle = confirmation === "delete" ? `Delete ${profile.name}?` : "Discard unsaved changes?";
  const confirmationCopy = confirmation === "delete" ? "This profile will no longer be available to switch sessions."
    : confirmation === "duplicate" ? "The duplicate will use the last saved version of this profile."
      : confirmation === "native" && nativeExitRequest === "quit" ? "Switchify PC will quit and your profile changes will be lost."
        : confirmation === "native" ? "The window will close and your profile changes will be lost."
          : "Your profile changes have not been saved.";
  const confirmationAction = confirmation === "delete" ? "Delete profile" : confirmation === "duplicate" ? "Discard and duplicate" : confirmation === "native" && nativeExitRequest === "quit" ? "Discard and quit" : confirmation === "native" ? "Discard and close" : "Discard changes";
  const cancelConfirmation = () => {
    if (confirmation === "native") onCancelNativeExit();
    setConfirmation(null);
  };
  const confirmAction = () => {
    if (confirmation === "delete") void runDelete();
    else if (confirmation === "duplicate") onDuplicate();
    else if (confirmation === "native") onConfirmNativeExit();
    else onClose();
  };

  if (confirmation) return <div className="modal-backdrop"><section ref={dialogRef} className="profile-dialog confirm-dialog" role="alertdialog" aria-modal="true" aria-labelledby="profile-confirm-title" tabIndex={-1} onKeyDown={trapFocus}>
    <header><div><h2 id="profile-confirm-title">{confirmationTitle}</h2><p>{confirmationCopy}</p></div></header>
    <footer><span /><button ref={confirmationButtonRef} className="secondary" onClick={cancelConfirmation}>Keep editing</button><button className={confirmation === "duplicate" ? "primary" : "primary danger"} disabled={busy} onClick={confirmAction}>{confirmationAction}</button></footer>
  </section></div>;

  return <div className="modal-backdrop"><section ref={dialogRef} className="profile-dialog" role="dialog" aria-modal="true" aria-labelledby="profile-title" tabIndex={-1} onKeyDown={trapFocus}>
    <header><div><h2 id="profile-title">{profile.builtIn ? profile.name : "Edit switch profile"}</h2><p>Map each physical switch to a desktop action.</p></div><button className="icon-button" title="Close" onClick={requestClose}><X size={18} /></button></header>
    {operationError && <div className="dialog-error" role="alert">{operationError}</div>}
    <label className="field"><span>Profile name</span><input data-error-key="name" value={draft.name} maxLength={50} disabled={profile.builtIn} aria-invalid={Boolean(errors.name)} aria-describedby={errors.name ? "profile-name-error" : undefined} onChange={(event) => setDraft({ ...draft, name: event.target.value })} />{errors.name && <span className="field-error" id="profile-name-error">{errors.name}</span>}</label>
    <div className="binding-list">{draft.bindings.map((binding, index) => <div className="binding-row" key={binding.switchId} data-invalid={Boolean(errors[`binding-${binding.switchId}`])}>
      <strong>Switch {binding.switchId}</strong>
      <select data-error-key={`binding-${binding.switchId}`} aria-label={`Switch ${binding.switchId} action`} aria-invalid={Boolean(errors[`binding-${binding.switchId}`])} aria-describedby={errors[`binding-${binding.switchId}`] ? `binding-${binding.switchId}-error` : undefined} value={binding.type} disabled={profile.builtIn} onChange={(event) => setBinding(index, event.target.value as typeof binding.type)}>
        <option value="none">No action</option><option value="key">Key</option><option value="shortcut">Shortcut</option><option value="mouseButton">Hold mouse button</option><option value="mouseClick">Mouse click</option><option value="scroll">Scroll</option><option value="media">Media</option>
      </select>
      {(binding.type === "key" || binding.type === "shortcut") && <input className="key-recorder" aria-label={`Switch ${binding.switchId} key`} placeholder="Select, then press key" value={bindingLabel(binding)} readOnly disabled={profile.builtIn} onKeyDown={(event) => recordKey(index, binding, event)} />}
      {(binding.type === "mouseButton" || binding.type === "mouseClick") && <select aria-label={`Switch ${binding.switchId} mouse button`} value={binding.value ?? "left"} disabled={profile.builtIn} onChange={(event) => setBinding(index, binding.type, event.target.value)}><option value="left">Left</option><option value="right">Right</option><option value="middle">Middle</option></select>}
      {binding.type === "scroll" && <select aria-label={`Switch ${binding.switchId} scroll direction`} value={binding.value ?? "down"} disabled={profile.builtIn} onChange={(event) => setBinding(index, binding.type, event.target.value)}><option value="up">Up</option><option value="down">Down</option><option value="left">Left</option><option value="right">Right</option></select>}
      {binding.type === "media" && <select aria-label={`Switch ${binding.switchId} media action`} value={binding.value ?? "playPause"} disabled={profile.builtIn} onChange={(event) => setBinding(index, binding.type, event.target.value)}><option value="playPause">Play / pause</option><option value="nextTrack">Next track</option><option value="previousTrack">Previous track</option><option value="volumeUp">Volume up</option><option value="volumeDown">Volume down</option><option value="mute">Mute</option></select>}
      {errors[`binding-${binding.switchId}`] && <span className="field-error binding-error" id={`binding-${binding.switchId}-error`}>{errors[`binding-${binding.switchId}`]}</span>}
    </div>)}</div>
    <footer>{onDelete && <button className="secondary danger" disabled={busy} onClick={() => setConfirmation("delete")}><Trash2 size={16} />Delete</button>}<button className="secondary" disabled={busy} onClick={() => dirty ? setConfirmation("duplicate") : onDuplicate()}><Copy size={16} />Duplicate</button><span /><button className="secondary" onClick={requestClose}>Cancel</button>{!profile.builtIn && <button className="primary" disabled={busy || Boolean(firstError)} onClick={() => void runSave()}><Save size={16} />Save profile</button>}</footer>
  </section></div>;
}

function ProfilesView({ profiles, platform, saveProfile, deleteProfile, onDirtyChange, nativeExitRequest, onConfirmNativeExit, onCancelNativeExit, busy }: { profiles: SwitchProfile[]; platform: AppState["capabilities"]["platform"]; saveProfile: (profile: SwitchProfile) => Promise<void>; deleteProfile: (id: string) => Promise<void>; onDirtyChange: (dirty: boolean) => void; nativeExitRequest: ProfileExitAction | null; onConfirmNativeExit: () => void; onCancelNativeExit: () => void; busy: boolean }) {
  const [editing, setEditing] = useState<SwitchProfile | null>(null);
  const openerRef = useRef<HTMLButtonElement | null>(null);
  const closeEditor = () => { setEditing(null); requestAnimationFrame(() => (openerRef.current?.isConnected ? openerRef.current : document.querySelector<HTMLButtonElement>(".page-header button"))?.focus()); };
  const openEditor = (profile: SwitchProfile, opener: HTMLButtonElement) => { openerRef.current = opener; setEditing(profile); };
  return <div className="view"><header className="page-header"><div><h1>Switch control</h1><p>Profiles available to physical switch sessions</p></div><button className="primary" onClick={(event) => openEditor(newProfile(), event.currentTarget)}><Plus size={16} />New profile</button></header>
    <div className="profile-list">{profiles.map((profile) => <button className="profile-row" key={profile.id} onClick={(event) => openEditor(profile, event.currentTarget)}><div className="profile-icon"><SlidersHorizontal size={19} /></div><div><h2>{profile.name}</h2><p>{profile.provider === "grid3" ? "Grid 3" : `${profile.bindings.filter((binding) => binding.type !== "none").length} mapped switches`}</p></div><span>{profile.builtIn ? "Built in" : "Custom"}</span><ChevronRight size={18} /></button>)}</div>
    {platform === "macos" && <p className="capability-note">Grid 3 profiles are available on Windows only.</p>}
    {editing && <ProfileEditor key={editing.id} profile={editing} profiles={profiles} busy={busy} onDirtyChange={onDirtyChange} nativeExitRequest={nativeExitRequest} onConfirmNativeExit={() => { closeEditor(); onConfirmNativeExit(); }} onCancelNativeExit={onCancelNativeExit} onClose={closeEditor} onDuplicate={() => setEditing(duplicateProfile(editing, profiles))} onSave={async (profile) => { await saveProfile(profile); closeEditor(); }} onDelete={editing.builtIn || !profiles.some((profile) => profile.id === editing.id) ? null : async () => { await deleteProfile(editing.id); closeEditor(); }} />}
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

const changedSettingKeys = (previous: AppSettings, next: AppSettings) =>
  (Object.keys(next) as Array<keyof AppSettings>).filter((key) => previous[key] !== next[key]);

function applyLocalSettings(base: AppSettings, local: AppSettings, keys: Set<keyof AppSettings>) {
  const merged = { ...base };
  for (const key of keys) Object.assign(merged, { [key]: local[key] });
  return merged;
}

type UpdateAction = "check" | "download" | "install";

function updateDescription(update: UpdateState) {
  switch (update.status) {
    case "unconfigured": return "Updates are unavailable in this build because its signed feed is not configured.";
    case "idle": return "Automatic update checks are enabled.";
    case "checking": return "Checking for updates…";
    case "available": return `Switchify PC ${update.version} is available.`;
    case "downloading": return `Downloading Switchify PC ${update.version}…`;
    case "readyToInstall": return `Switchify PC ${update.version} is ready to install.`;
    case "applying": return `Installing Switchify PC ${update.version}…`;
    case "current": return "Switchify PC is up to date.";
    case "failed": return update.error ?? "The update operation failed.";
    case "cancelled": return "Download cancelled. You can resume when ready.";
  }
}

function UpdateControls({ update, run, cancel }: { update: UpdateState; run: (action: UpdateAction) => void; cancel: () => void }) {
  const percent = update.totalBytes && update.totalBytes > 0
    ? Math.min(100, Math.round(update.downloadedBytes * 100 / update.totalBytes))
    : null;
  const action = update.status === "available" || update.status === "cancelled" ? "download"
    : update.status === "readyToInstall" ? "install"
      : update.status === "failed" ? update.retryAction
        : update.status === "idle" || update.status === "current" || update.status === "unconfigured" ? "check" : null;
  const label = update.status === "failed" ? "Retry"
    : action === "download" ? (update.status === "cancelled" ? "Resume download" : "Download")
      : action === "install" ? "Install and restart" : "Check for updates";
  return <div className="update-controls">
    <p role={update.status === "failed" ? "alert" : "status"}>{updateDescription(update)}</p>
    {update.status === "downloading" && <>
      <progress aria-label="Update download progress" value={update.downloadedBytes} max={update.totalBytes ?? undefined} />
      <span>{percent === null ? `${update.downloadedBytes.toLocaleString()} bytes` : `${percent}%`}</span>
    </>}
    <div>{action && <button className="secondary" type="button" onClick={() => run(action)}>{action === "download" && <Download size={16} />}{action === "check" && <RefreshCw size={16} />}{label}</button>}{update.status === "downloading" && <button className="secondary" type="button" onClick={cancel}><X size={16} />Cancel</button>}{(update.status === "checking" || update.status === "applying") && <button className="secondary" type="button" disabled><RefreshCw className="spin" size={16} />{update.status === "checking" ? "Checking" : "Installing"}</button>}</div>
  </div>;
}

function SettingsView({ state, settings, onChange, chooseTelemetry, updateAction, cancelUpdate, busy }: { state: AppState; settings: AppSettings; onChange: (next: AppSettings) => void; chooseTelemetry: (enabled: boolean) => void; updateAction: (action: UpdateAction) => void; cancelUpdate: () => void; busy: boolean }) {
  const update = <K extends keyof AppSettings>(key: K, value: AppSettings[K]) => onChange({ ...settings, [key]: value });
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
    <SettingGroup title="Privacy" description="Optional anonymous app health and sanitized error reports. Never includes typed text, commands, pairing secrets, device names, or full paths."><Toggle label="Share anonymous diagnostic data" disabled={!state.telemetry.available && !settings.shareDiagnostics} checked={settings.shareDiagnostics} onChange={(value) => update("shareDiagnostics", value)} />{state.telemetry.consent === "undecided" && <div className="privacy-choice" role="group" aria-label="Anonymous diagnostics choice"><button className="secondary" type="button" disabled={busy || !state.telemetry.available} onClick={() => chooseTelemetry(true)}>Share diagnostics</button><button className="secondary" type="button" disabled={busy} onClick={() => chooseTelemetry(false)}>Don't share</button></div>}<p className="setting-note">{state.telemetry.available ? state.telemetry.consent === "undecided" ? "No choice recorded yet. Nothing is sent unless you choose Share diagnostics." : state.telemetry.consent === "enabled" ? "Consent recorded. You can turn this off at any time to delete queued reports." : "Opted out. No diagnostic reports are stored or sent." : "Diagnostic reporting is unavailable in this build."} <a href="https://switchifyapp.com/privacy" target="_blank" rel="noreferrer">Privacy policy</a></p></SettingGroup>
    <SettingGroup title="Updates" description={`Switchify PC ${state.version}`}><UpdateControls update={state.updater} run={updateAction} cancel={cancelUpdate} /></SettingGroup>
  </div>;
}

function SupportView({ state, busy, perform, openSetup }: { state: AppState; busy: boolean; perform: (operation: () => Promise<AppState>) => void; openSetup: () => void }) {
  const [tab, setTab] = useState<"setup" | "troubleshooting">("setup");
  const bluetoothReady = state.bluetooth === "advertising" || state.bluetooth === "connected";
  return <div className="view"><header className="page-header"><div><h1>Support</h1><p>Connection setup and system diagnostics</p></div><CircleHelp size={24} /></header>
    <div className="segmented" role="tablist" aria-label="Support view"><button role="tab" aria-selected={tab === "setup"} onClick={() => setTab("setup")}>Setup</button><button role="tab" aria-selected={tab === "troubleshooting"} onClick={() => setTab("troubleshooting")}>Troubleshooting</button></div>
    {tab === "setup" ? <><button className="primary setup-launch" onClick={openSetup}><Wrench size={16} />Open setup guide</button><section className="task-list" aria-label="Setup status">
      <article><StatusIcon ok={bluetoothReady}>{bluetoothReady ? <CheckCircle2 size={19} /> : <Bluetooth size={19} />}</StatusIcon><div><h2>Bluetooth</h2><p>{bluetoothLabels[state.bluetooth]}</p></div></article>
      <article><StatusIcon ok={state.accessibility === "granted"}><Accessibility size={19} /></StatusIcon><div><h2>Input access</h2><AccessibilityCopy state={state} detailed /></div>{state.accessibility === "required" && <button className="secondary" disabled={busy} onClick={() => perform(() => api.checkAccessibility(true))}>Open Accessibility Settings</button>}</article>
      <article><StatusIcon ok={state.pairedDevices.length > 0}><Smartphone size={19} /></StatusIcon><div><h2>Android device</h2><p>{state.pairedDevices.length > 0 ? `${state.pairedDevices.length} paired` : "Open Switchify on Android and select this computer"}</p></div></article>
      <article><StatusIcon ok={state.bluetooth === "connected"}><Radio size={19} /></StatusIcon><div><h2>Connection</h2><p>{state.connectedDeviceName ?? "Waiting for a paired device"}</p></div></article>
    </section></> : <section className="task-list" aria-label="Troubleshooting actions">
      <article><Bluetooth size={20} /><div><h2>Bluetooth connection</h2><p>{bluetoothLabels[state.bluetooth]}</p></div><button className="secondary" disabled={busy} onClick={() => perform(api.disconnectAll)}><Power size={16} />Disconnect</button></article>
      <article><Accessibility size={20} /><div><h2>Input access</h2><AccessibilityCopy state={state} detailed /></div>{state.accessibility === "required" ? <button className="secondary" disabled={busy} onClick={() => perform(() => api.checkAccessibility(true))}>Open Accessibility Settings</button> : <button className="secondary" disabled={busy} onClick={() => perform(() => api.checkAccessibility(false))}><RefreshCw size={16} />Check input access</button>}</article>
      <article><RefreshCw size={20} /><div><h2>Application update</h2><p>Switchify PC {state.version}</p></div><button className="secondary" disabled={busy} onClick={() => perform(api.checkForUpdates)}>Check</button></article>
      <article><Download size={20} /><div><h2>Diagnostics</h2><p>Export sanitized health, capability, and recent event data</p></div><button className="secondary" disabled={busy} onClick={() => perform(api.exportDiagnostics)}><Download size={16} />Export</button></article>
      <article className="diagnostic-detail"><Bluetooth size={20} /><div><h2>Recent Bluetooth changes</h2><p>{state.diagnostics.recentBluetooth.length > 0 ? state.diagnostics.recentBluetooth.map((event) => event.status).join(" → ") : "No Bluetooth changes recorded yet"}</p></div></article>
      <article className="diagnostic-detail"><Power size={20} /><div><h2>Last disconnect</h2><p>{state.diagnostics.lastDisconnect ? `${state.diagnostics.lastDisconnect.detail ?? state.diagnostics.lastDisconnect.status}` : "No disconnect recorded yet"}</p></div></article>
      <article className="diagnostic-detail"><CircleHelp size={20} /><div><h2>Recent errors</h2><p>{state.diagnostics.recentErrors.length > 0 ? state.diagnostics.recentErrors.map((event) => event.detail ?? event.status).join(" · ") : "No recent errors"}</p></div></article>
    </section>}
    {state.lastActivity && <section className="activity-panel" aria-live="polite"><span>Recent activity</span><p data-kind={state.lastActivity.kind}>{state.lastActivity.message}</p></section>}
  </div>;
}

function SetupGuide({ state, busy, error, skip, finish, accessibility, approve, reject }: {
  state: AppState;
  busy: boolean;
  error: string | null;
  skip: () => Promise<void>;
  finish: (startWithSystem: boolean, shareDiagnostics: boolean) => Promise<void>;
  accessibility: () => Promise<void>;
  approve: (requestId: string) => Promise<void>;
  reject: (requestId: string) => Promise<void>;
}) {
  const [step, setStep] = useState(0);
  const [startupChoice, setStartupChoice] = useState<boolean | null>(state.setup.completed ? state.settings.startWithSystem : null);
  const [diagnosticsChoice, setDiagnosticsChoice] = useState<boolean | null>(state.setup.completed && state.telemetry.consent !== "undecided" ? state.telemetry.consent === "enabled" : null);
  const dialogRef = useRef<HTMLElement>(null);
  const titles = ["Bluetooth and input access", "Get Switchify for Android", "Pair securely", "Start with system", "Anonymous diagnostics"];
  const bluetoothReady = state.bluetooth === "advertising" || state.bluetooth === "connected";
  const canContinue = step === 2 ? state.pairedDevices.length > 0 : step === 3 ? startupChoice !== null : step === 4 ? diagnosticsChoice !== null : true;

  useEffect(() => { dialogRef.current?.focus(); }, [step]);

  const trapFocus = (event: React.KeyboardEvent<HTMLElement>) => {
    if (event.key !== "Tab") return;
    const controls = [...(dialogRef.current?.querySelectorAll<HTMLElement>('button:not(:disabled), a[href]') ?? [])];
    if (controls.length === 0) return;
    const first = controls[0]; const last = controls.at(-1)!;
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
  };

  return <div className="modal-backdrop setup-backdrop"><section ref={dialogRef} className="setup-dialog" role="dialog" aria-modal="true" aria-labelledby="setup-title" tabIndex={-1} onKeyDown={trapFocus}>
    <header><div><span>Setup guide</span><h2 id="setup-title">{titles[step]}</h2></div><strong aria-label={`Step ${step + 1} of 5`}>{step + 1} / 5</strong></header>
    <div className="setup-progress" aria-hidden="true">{titles.map((title, index) => <span key={title} data-active={index <= step} />)}</div>
    {error && <div className="dialog-error" role="alert">{error}</div>}
    <div className="setup-content">
      {step === 0 && <div className="setup-statuses">
        <article><StatusIcon ok={bluetoothReady}><Bluetooth size={19} /></StatusIcon><div><h3>Bluetooth</h3><p>{bluetoothLabels[state.bluetooth]}</p></div></article>
        <article><StatusIcon ok={state.accessibility === "granted"}><Accessibility size={19} /></StatusIcon><div><h3>Input access</h3><AccessibilityCopy state={state} detailed /></div>{state.accessibility === "required" && <button className="secondary" disabled={busy} onClick={() => void accessibility()}>Open Accessibility Settings</button>}</article>
      </div>}
      {step === 1 && <div className="android-download"><div><h3>Install the Android app</h3><p>Install Switchify from Google Play, then open it near this computer.</p><a className="secondary" href={androidDownloadUrl} target="_blank" rel="noreferrer">Open Google Play</a></div><img src={androidQrUrl} alt="QR code for Switchify on Google Play" /></div>}
      {step === 2 && <div><h3>{state.pairedDevices.length > 0 ? "Android device paired" : "Waiting for an Android device"}</h3><p>{state.pairedDevices.length > 0 ? "Secure pairing is complete. You can continue setup." : "In Switchify for Android, select this computer and confirm the matching code."}</p>
        {state.pendingPairings.length > 0 && <div className="setup-pairings" aria-label="Pending pairing requests">{state.pendingPairings.map((request) => <article key={request.requestId}><div><strong>{request.deviceName}</strong><span>Verification code</span></div><output aria-label={`Verification code for ${request.deviceName}`}>{request.verificationCode}</output><div><button className="secondary danger" disabled={busy} aria-label={`Reject pairing request from ${request.deviceName}, code ${request.verificationCode}`} onClick={() => void reject(request.requestId)}>Reject</button><button className="primary" disabled={busy} aria-label={`Accept pairing request from ${request.deviceName}, code ${request.verificationCode}`} onClick={() => void approve(request.requestId)}>Accept</button></div></article>)}</div>}
      </div>}
      {step === 3 && <div><h3>Choose startup behavior</h3><p>Switchify can start quietly when you sign in, ready for your Android device.</p><div className="setup-choices" role="group" aria-label="Start with system choice"><button className="secondary" aria-pressed={startupChoice === true} onClick={() => setStartupChoice(true)}>Start with system</button><button className="secondary" aria-pressed={startupChoice === false} onClick={() => setStartupChoice(false)}>Start manually</button></div></div>}
      {step === 4 && <div><h3>Choose whether to share diagnostics</h3><p>Optional anonymous app health and sanitized errors help improve Switchify. Typed text, commands, pairing secrets, device names, and full paths are never included.</p><div className="setup-choices" role="group" aria-label="Anonymous diagnostics choice"><button className="secondary" disabled={!state.telemetry.available} aria-pressed={diagnosticsChoice === true} onClick={() => setDiagnosticsChoice(true)}>Share diagnostics</button><button className="secondary" aria-pressed={diagnosticsChoice === false} onClick={() => setDiagnosticsChoice(false)}>Don’t share</button></div><a className="setup-privacy" href="https://switchifyapp.com/privacy" target="_blank" rel="noreferrer">Privacy policy</a></div>}
    </div>
    <footer><button className="text-button" disabled={busy} onClick={() => void skip()}>Skip for now</button><span /><button className="secondary" disabled={busy || step === 0} onClick={() => setStep((current) => current - 1)}>Back</button><button className="primary" disabled={busy || !canContinue} onClick={() => step === 4 ? void finish(startupChoice!, diagnosticsChoice!) : setStep((current) => current + 1)}>{step === 4 ? "Finish" : "Next"}</button></footer>
  </section></div>;
}

function PairingDialog({ requests, connectedDeviceName, busy, approve, reject }: {
  requests: PendingPairing[];
  connectedDeviceName: string | null;
  busy: boolean;
  approve: (requestId: string) => Promise<void>;
  reject: (requestId: string) => Promise<void>;
}) {
  const dialogRef = useRef<HTMLElement>(null);
  const previousFocus = useRef<HTMLElement | null>(null);
  const actedRequest = useRef<string | null>(null);

  useEffect(() => {
    previousFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    dialogRef.current?.focus();
    return () => previousFocus.current?.focus();
  }, []);

  useEffect(() => {
    if (!actedRequest.current || requests.some((request) => request.requestId === actedRequest.current)) return;
    actedRequest.current = null;
    dialogRef.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }, [requests]);

  const run = (requestId: string, operation: (id: string) => Promise<void>) => {
    actedRequest.current = requestId;
    void operation(requestId);
  };

  const trapFocus = (event: React.KeyboardEvent<HTMLElement>) => {
    if (event.key !== "Tab") return;
    const controls = [...(dialogRef.current?.querySelectorAll<HTMLElement>("button:not(:disabled)") ?? [])];
    if (controls.length === 0) {
      event.preventDefault();
      dialogRef.current?.focus();
      return;
    }
    const first = controls[0];
    const last = controls.at(-1)!;
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  };

  return <div className="modal-backdrop"><section ref={dialogRef} className="pairing-dialog" role="dialog" aria-modal="true" aria-labelledby="pairing-title" tabIndex={-1} onKeyDown={trapFocus}>
    <header><Smartphone size={26} /><div><h2 id="pairing-title">Pairing requests</h2><p>Confirm each code matches Switchify for Android.</p>{connectedDeviceName && <p className="pairing-connection">Connected to {connectedDeviceName}</p>}</div><span>{requests.length}</span></header>
    <div className="pairing-list" aria-label="Pending pairing requests">
      {requests.map((request, index) => {
        const titleId = `pairing-request-${index}`;
        const actionDescription = `${request.deviceName}, code ${request.verificationCode}`;
        return <article key={request.requestId} aria-labelledby={titleId}>
          <div><h3 id={titleId}>{request.deviceName}</h3><p>Verification code</p></div>
          <output aria-label={`Verification code for ${request.deviceName}`}>{request.verificationCode}</output>
          <div className="pairing-actions"><button className="secondary danger" disabled={busy} aria-label={`Reject pairing request from ${actionDescription}`} onClick={() => run(request.requestId, reject)}>Reject</button><button className="primary" disabled={busy} aria-label={`Accept pairing request from ${actionDescription}`} onClick={() => run(request.requestId, approve)}>Accept</button></div>
        </article>;
      })}
    </div>
  </section></div>;
}

export function App() {
  const [state, setState] = useState<AppState | null>(null);
  const [view, setView] = useState<View>("home");
  const viewRef = useRef<View>("home");
  const [profiles, setProfiles] = useState<SwitchProfile[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [busy, setBusy] = useState(false);
  const [checkingUpdates, setCheckingUpdates] = useState(false);
  const [setupOpen, setSetupOpen] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [profileExitRequest, setProfileExitRequest] = useState<ProfileExitAction | null>(null);
  const settingsDirty = useRef(false);
  const confirmedSettings = useRef<AppSettings | null>(null);
  const displayedSettings = useRef<AppSettings | null>(null);
  const pendingSettings = useRef<AppSettings | null>(null);
  const settingsSaveRunning = useRef(false);
  const settingsEventRevision = useRef(0);
  const locallyChangedSettings = useRef(new Set<keyof AppSettings>());
  const profileEditorDirty = useRef(false);
  const autoSetupHandled = useRef(false);

  const syncState = (next: AppState) => {
    setState(next);
    if (!settingsDirty.current) {
      confirmedSettings.current = next.settings;
      displayedSettings.current = next.settings;
      setSettings(next.settings);
    } else if (!confirmedSettings.current || changedSettingKeys(confirmedSettings.current, next.settings).length > 0) {
      settingsEventRevision.current += 1;
      if (confirmedSettings.current) {
        for (const key of changedSettingKeys(confirmedSettings.current, next.settings)) {
          locallyChangedSettings.current.delete(key);
        }
      }
      confirmedSettings.current = next.settings;
      const rebased = applyLocalSettings(
        next.settings,
        displayedSettings.current ?? next.settings,
        locallyChangedSettings.current,
      );
      pendingSettings.current = rebased;
      displayedSettings.current = rebased;
      setSettings(rebased);
    }
  };

  const perform = async (operation: () => Promise<AppState>) => {
    setBusy(true); setError(null);
    try { syncState(await operation()); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  };

  const processSettingsSaves = async () => {
    if (settingsSaveRunning.current) return;
    settingsSaveRunning.current = true;
    try {
      while (pendingSettings.current) {
        const requested = pendingSettings.current;
        const requestRevision = settingsEventRevision.current;
        pendingSettings.current = null;
        try {
          const next = await api.saveSettings(requested);
          if (requestRevision === settingsEventRevision.current) {
            confirmedSettings.current = next.settings;
            setState((current) => current ? { ...current, settings: next.settings } : next);
          }
          if (!pendingSettings.current) {
            settingsDirty.current = false;
            locallyChangedSettings.current.clear();
            const confirmed = confirmedSettings.current ?? next.settings;
            displayedSettings.current = confirmed;
            setSettings(confirmed);
          }
        } catch (reason) {
          pendingSettings.current = null;
          settingsDirty.current = false;
          locallyChangedSettings.current.clear();
          if (confirmedSettings.current) {
            displayedSettings.current = confirmedSettings.current;
            setSettings(confirmedSettings.current);
          }
          setError(String(reason));
          break;
        }
      }
    } finally {
      settingsSaveRunning.current = false;
    }
  };

  const changeSettings = (next: AppSettings) => {
    const current = displayedSettings.current;
    if (current) {
      for (const key of changedSettingKeys(current, next)) locallyChangedSettings.current.add(key);
    }
    settingsDirty.current = true;
    pendingSettings.current = next;
    displayedSettings.current = next;
    setSettings(next);
    setError(null);
    void processSettingsSaves();
  };

  const checkForUpdates = async () => {
    setCheckingUpdates(true);
    try { await perform(api.checkForUpdates); }
    finally { setCheckingUpdates(false); }
  };

  const runUpdate = async (action: UpdateAction) => {
    setError(null);
    if (action === "check") setCheckingUpdates(true);
    try {
      const operation = action === "check" ? api.checkForUpdates : action === "download" ? api.downloadUpdate : api.installUpdate;
      syncState(await operation());
    } catch (reason) { setError(String(reason)); }
    finally { if (action === "check") setCheckingUpdates(false); }
  };

  const cancelUpdate = async () => {
    setError(null);
    try { syncState(await api.cancelUpdateDownload()); }
    catch (reason) { setError(String(reason)); }
  };

  const openSetup = () => {
    setSetupOpen(true);
    void perform(api.markSetupShown);
  };

  const finishSetup = async (startWithSystem: boolean, shareDiagnostics: boolean) => {
    setBusy(true); setError(null);
    try {
      syncState(await api.completeSetup(startWithSystem, shareDiagnostics));
      setSetupOpen(false);
    } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  };

  const skipSetup = async () => {
    setBusy(true); setError(null);
    try {
      syncState(await api.markSetupShown());
      setSetupOpen(false);
    } catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  };

  useEffect(() => {
    let unlisten: () => void = () => {};
    void api.state().then(syncState).catch((reason) => setError(String(reason)));
    void api.onState(syncState).then((stop) => { unlisten = stop; });
    return () => unlisten();
  }, []);

  useEffect(() => {
    let unlisten: () => void = () => {};
    let disposed = false;
    void api.onNavigateRequested((target) => {
      if (!disposed) selectView(target);
    }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => { disposed = true; unlisten(); };
  }, []);

  useEffect(() => {
    if (!state || autoSetupHandled.current) return;
    autoSetupHandled.current = true;
    if (state.setup.autoOpenEligible) openSetup();
  }, [state]);

  useEffect(() => {
    let unlisten: () => void = () => {};
    void api.onProfileExitRequested((action) => {
      if (profileEditorDirty.current) setProfileExitRequest(action);
      else void api.completeProfileExit().catch((reason) => setError(String(reason)));
    }).then((stop) => { unlisten = stop; });
    return () => unlisten();
  }, []);

  useEffect(() => {
    const preventUnsavedUnload = (event: BeforeUnloadEvent) => {
      if (!profileEditorDirty.current) return;
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", preventUnsavedUnload);
    return () => window.removeEventListener("beforeunload", preventUnsavedUnload);
  }, []);

  useEffect(() => { if (view === "profiles") void api.listProfiles().then(setProfiles).catch((reason) => setError(String(reason))); }, [view]);
  const nav = useMemo(() => [
    ["home", "Home", <Home size={19} />], ["devices", "Devices", <Smartphone size={19} />],
    ["profiles", "Switch control", <Keyboard size={19} />], ["settings", "Settings", <Settings size={19} />],
    ["support", "Support", <CircleHelp size={19} />],
  ] as const, []);

  const selectView = (next: View) => {
    if (next === viewRef.current) return;
    if (profileEditorDirty.current && !window.confirm("Discard unsaved profile changes?")) return;
    profileEditorDirty.current = false;
    viewRef.current = next;
    setView(next);
  };

  const saveProfile = async (profile: SwitchProfile) => {
    setBusy(true); setError(null);
    try { setProfiles(await api.saveProfile(profile)); }
    catch (reason) { throw reason; }
    finally { setBusy(false); }
  };

  const deleteProfile = async (id: string) => {
    setBusy(true); setError(null);
    try { setProfiles(await api.deleteProfile(id)); }
    catch (reason) { throw reason; }
    finally { setBusy(false); }
  };

  const cancelProfileExit = () => {
    setProfileExitRequest(null);
    void api.cancelProfileExit().catch((reason) => setError(String(reason)));
  };

  const confirmProfileExit = () => {
    setProfileExitRequest(null);
    profileEditorDirty.current = false;
    void api.completeProfileExit().catch((reason) => setError(String(reason)));
  };

  if (!state || !settings) return <div className="loading"><RefreshCw className="spin" size={24} /><span>Starting Switchify PC...</span></div>;
  return <div className="app-shell">
    <aside><div className="brand"><img className="brand-mark" src={brandIconUrl} alt="" aria-hidden="true" /><div><strong>Switchify</strong><small>PC</small></div></div><nav>{nav.map(([id, label, icon]) => <NavButton key={id} active={view === id} icon={icon} onClick={() => selectView(id)}>{label}</NavButton>)}</nav><div className="sidebar-footer"><span>v{state.version}</span><button className="footer-update-button" type="button" aria-label="Check for updates" title="Check for updates" disabled={busy} onClick={() => void checkForUpdates()}><RefreshCw className={checkingUpdates ? "spin" : undefined} size={15} /></button></div></aside>
    <main>
      {error && <div className="error-banner" role="alert">{error}<button onClick={() => setError(null)}>Dismiss</button></div>}
      {view === "home" && <HomeView state={state} onDisconnect={() => void perform(api.disconnectAll)} onAccessibility={() => void perform(() => api.checkAccessibility(true))} onSetup={openSetup} />}
      {view === "devices" && <DevicesView state={state} forget={(id) => void perform(() => api.forgetDevice(id))} />}
      {view === "profiles" && <ProfilesView profiles={profiles} platform={state.capabilities.platform} busy={busy} saveProfile={saveProfile} deleteProfile={deleteProfile} onDirtyChange={(dirty) => { profileEditorDirty.current = dirty; }} nativeExitRequest={profileExitRequest} onConfirmNativeExit={confirmProfileExit} onCancelNativeExit={cancelProfileExit} />}
      {view === "settings" && <SettingsView state={state} settings={settings} onChange={changeSettings} chooseTelemetry={(enabled) => void perform(() => api.setTelemetryConsent(enabled))} updateAction={(action) => void runUpdate(action)} cancelUpdate={() => void cancelUpdate()} busy={busy} />}
      {view === "support" && <SupportView state={state} busy={busy} perform={(operation) => void perform(operation)} openSetup={openSetup} />}
    </main>
    {setupOpen && <SetupGuide state={state} busy={busy} error={error} skip={skipSetup} finish={finishSetup} accessibility={() => perform(() => api.checkAccessibility(true))} reject={(requestId) => perform(() => api.rejectPairing(requestId))} approve={(requestId) => perform(() => api.approvePairing(requestId))} />}
    {!setupOpen && state.pendingPairings.length > 0 && <PairingDialog requests={state.pendingPairings} connectedDeviceName={state.connectedDeviceName} busy={busy} reject={(requestId) => perform(() => api.rejectPairing(requestId))} approve={(requestId) => perform(() => api.approvePairing(requestId))} />}
  </div>;
}
