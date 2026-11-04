import { Button, Input, Select, StatusIcon } from "./ui/controls";
import { Demonstration, DemonstrationProvider } from "./help/Demonstration";
import { useEffect, useId, useMemo, useRef, useState, type ReactNode } from "react";
import {
  Accessibility, Bluetooth, ChevronRight, CircleHelp, Download,
  Copy, Home, Keyboard, KeyRound, LayoutGrid, Plus, Power, RefreshCw, Save, Settings,
  SlidersHorizontal, Smartphone, Trash2, Wrench, X,
} from "lucide-react";
import { api, type NavigationTarget, type ProfileExitAction } from "./api";
import type { AccountView, AppSettings, AppState, PendingPairing, SwitchProfile, UpdateState } from "./types";
import { applyLocalSettings, changedSettingKeys } from "./settings/diff";
import { SettingsView } from "./settings/SettingsView";
import { CursorSection } from "./settings/CursorSection";
import { updateDescription, updateInFlight, updateLiveness, updateProgress, updateStanding, useUpdateProgress, type UpdateAction } from "./settings/UpdatesSection";
import { PanelsView } from "./panels/PanelsView";
import { usePanels } from "./panels/usePanels";
import { useSwitches, type SwitchController } from "./scanning/useSwitches";
import { lastModeName, selectOpensHome, useScanning, type ScanningController } from "./scanning/useScanning";
import { areaOptions, type ScanArea } from "./scanning/preferences";
import { homeStatus } from "./homeStatus";
import { BuyLicence } from "./licensing/BuyLicence";
import { interfaceModeAdvice, reservedKeyAdvice } from "./help/keyAvailability";
import { SwitchesSection } from "./settings/SwitchesSection";
import { ScanningSection } from "./settings/ScanningSection";
import { AlsoSee, CommonTasks, FindResults, FindTile, PageHead, RecentlyChanged, SectionStrip, useArrival, useRecent, type Jump } from "./find/Find";
import { pageLabels, type FindContext, type Page, type Target } from "./find/index";

type View = Page;

/** The pages, in nav order, with the icon each tile shows. */
const pages: [Page, ReactNode][] = [
  ["home", <Home size={24} />], ["switches", <Keyboard size={24} />], ["scanning", <SlidersHorizontal size={24} />], ["panels", <LayoutGrid size={24} />],
  ["mobile", <Smartphone size={24} />], ["settings", <Settings size={24} />], ["support", <CircleHelp size={24} />],
];

const brandIconUrl = new URL("../src-tauri/icons/icon.png", import.meta.url).href;
const mobileQrUrl = new URL("./assets/mobile-download-qr.png", import.meta.url).href;
const mobileDownloadUrl = "https://play.google.com/store/apps/details?id=com.enaboapps.switchify";

const bluetoothLabels: Record<AppState["bluetooth"], string> = {
  initializing: "Starting Bluetooth...", advertising: "Ready to connect", connected: "Device connected",
  poweredOff: "Bluetooth is off", unauthorized: "Bluetooth permission required",
  conflict: "Current Switchify PC is running", unsupported: "Bluetooth unavailable", error: "Bluetooth unavailable",
};

const bluetoothDescriptions: Record<AppState["bluetooth"], string> = {
  initializing: "Preparing this computer for nearby devices.",
  advertising: "Waiting for a nearby mobile device.",
  connected: "Mobile device connected.",
  poweredOff: "Turn on Bluetooth to connect a mobile device.",
  unauthorized: "Allow Bluetooth access in System Settings to connect.",
  conflict: "Quit the other Switchify PC instance, then reopen this app.",
  unsupported: "This computer does not support the required Bluetooth features.",
  error: "Bluetooth could not start. Try restarting Switchify PC.",
};

function AccessibilityCopy({ state, detailed = false }: { state: AppState; detailed?: boolean }) {
  if (state.accessibility === "granted") return <p>Ready</p>;
  if (state.accessibility === "unavailable") return <p>Unavailable on this system</p>;
  if (!detailed || state.capabilities.platform !== "macos") return <p>Permission required</p>;
  return <>
    <p>Enable “Switchify PC” in Accessibility, then return to the app. Status updates automatically.</p>
    <p className="permission-recovery">If it is already enabled but access is still required, select the stale row, click Remove, return to Switchify, reopen Accessibility Settings, and enable the newly added entry.</p>
  </>;
}

function HomeView({ state, switches, scanning, navigate, ctx, jump, recent, onDisconnect, onAccessibility, onReleaseSwitchKeys }: { state: AppState; switches: SwitchController; scanning: ScanningController; navigate: (view: View) => void; ctx: FindContext; jump: Jump; recent: string[]; onDisconnect: () => void; onAccessibility: () => void; onReleaseSwitchKeys: (released: boolean) => void }) {
  const saved = switches.state?.settings.bindings ?? [];
  const hasSelect = saved.some((binding) => binding.pressAction === "select" || binding.holdActions.includes("select"));
  // The hero names the state in words, so its colour is never the only cue.
  const { title, tone, message } = homeStatus({ accessibility: state.accessibility, bluetooth: state.bluetooth, switchesLoaded: !!switches.state, hasSelect, error: switches.error ?? scanning.error, scanning: scanning.state, switchKeysReleased: state.switchKeysReleased });
  const ready = tone === "ready";
  const mobileConnected = state.bluetooth === "connected";
  const released = state.switchKeysReleased;
  return <div className="view home-view">
    <PageHead title="Switchify PC" blurb="Control your computer with switches." />
    <section className="connection-band status-hero" data-tone={tone} aria-labelledby="home-status-title">
      <StatusIcon ok={ready ? true : tone === "attention" ? false : undefined}><Keyboard size={26} /></StatusIcon>
      <div><h2 id="home-status-title">{title}</h2><p role="status">{message}</p></div>
      {released
        ? <Button className="primary" onClick={() => onReleaseSwitchKeys(false)}>Resume listening</Button>
        : <Button className={ready ? "secondary" : "primary"} onClick={() => navigate("switches")}>{hasSelect ? "Edit switches" : "Set up switches"}</Button>}
    </section>
    <CommonTasks ctx={ctx} jump={jump} />
    <section className="home-health" aria-labelledby="home-health-heading">
    <h2 id="home-health-heading">Your setup</h2>
    <section className="status-list" aria-label="Switch control status">
      <article><StatusIcon ok={state.accessibility === "granted"}><Accessibility size={19} /></StatusIcon><div><h3>Input access</h3><AccessibilityCopy state={state} /></div>{state.accessibility === "required" && <Button className="text-button" onClick={onAccessibility}>Open Accessibility Settings</Button>}</article>
      <article><StatusIcon ok={released ? undefined : hasSelect}><Keyboard size={19} /></StatusIcon><div><h3>Saved switches</h3><p>{saved.length} saved. {released ? "Listening paused" : hasSelect ? "Select assigned" : "Select action needed"}</p></div>{saved.length > 0 && <Button className="text-button" onClick={() => onReleaseSwitchKeys(!released)}>{released ? "Resume listening" : "Pause listening"}</Button>}</article>
      <article><StatusIcon><SlidersHorizontal size={19} /></StatusIcon><div><h3>Scanning</h3><p>{selectOpensHome(scanning.config) ? "Select opens Home" : `Select starts ${lastModeName(scanning.config)}`}. {areaOptions(scanning.config, scanning.config.controlMode).automatic ? "Automatic" : "Manual"}</p></div><Button className="text-button" onClick={() => navigate("scanning")}>Change scanning</Button></article>
      <article><StatusIcon ok={mobileConnected || undefined}><Smartphone size={19} /></StatusIcon><div><h3>Mobile connection (optional)</h3><p>{mobileConnected ? state.connectedDeviceName ?? bluetoothLabels.connected : bluetoothLabels[state.bluetooth]}</p></div>{mobileConnected ? <Button className="secondary" onClick={onDisconnect}>Disconnect</Button> : <Button className="text-button" onClick={() => navigate("mobile")}>Connect mobile</Button>}</article>
    </section>
    </section>
    <RecentlyChanged ctx={ctx} jump={jump} recent={recent} />
  </div>;
}

function MobileConnection({ state, onDisconnect }: { state: AppState; onDisconnect: () => void }) {
  const bluetoothOk = state.bluetooth === "advertising" || state.bluetooth === "connected";
  // Amber only when the person can do something about it here.
  const bluetoothFixable = state.bluetooth === "poweredOff" || state.bluetooth === "unauthorized" || state.bluetooth === "conflict";
  return <section className="subview" data-section="connection" aria-label="Connection">
    <section className="connection-band" data-tone={state.bluetooth === "connected" ? "ready" : bluetoothFixable ? "attention" : "neutral"} data-setting="connection">
      <StatusIcon ok={bluetoothOk || (bluetoothFixable ? false : undefined)}><Bluetooth size={20} /></StatusIcon><div><h2>{bluetoothLabels[state.bluetooth]}</h2><p>{state.bluetooth === "connected" ? state.connectedDeviceName ?? bluetoothDescriptions.connected : bluetoothDescriptions[state.bluetooth]}</p></div>
      {state.bluetooth === "connected" && <Button className="secondary" onClick={onDisconnect}>Disconnect</Button>}
    </section>
    <p className="mobile-note">Optional. Connecting a mobile device pauses local scanning.</p>
    <div className="mobile-cards" data-section="pairing" data-setting="pairing">
    <div className="mobile-download">
      <h2>Install Switchify for mobile</h2><p>Scan to get the app.</p>
      <figure><img src={mobileQrUrl} alt="QR code for Switchify on Google Play" /><figcaption>Get Switchify</figcaption></figure>
      <a className="secondary" href={mobileDownloadUrl} target="_blank" rel="noreferrer">Open Google Play</a>
    </div>
    <section className="mobile-pairing"><h2>Connect your mobile</h2><ol><li>Open Switchify on your mobile device and select this computer.</li><li>Compare the pairing codes shown in both apps.</li><li>Approve the pairing request on this computer only if the codes match.</li></ol><Demonstration kind="pair" /></section>
    </div>
  </section>;
}

function DevicesView({ state, forget }: { state: AppState; forget: (id: string) => void }) {
  return <div className="subview" data-section="devices" data-setting="devices"><header className="section-header"><div><h2>Paired devices</h2><p>Mobile devices trusted by this computer</p></div></header>
    {state.pairedDevices.length === 0 ? <div className="empty-state"><Smartphone size={28} /><h3>No paired devices</h3><p>Open Switchify on your mobile device to pair while this computer is advertising.</p></div> :
      <div className="device-list">{state.pairedDevices.map((device) => <article key={device.deviceId}><StatusIcon><Smartphone size={19} /></StatusIcon><div><h3>{device.deviceName}</h3><p>{device.lastSeenAt !== null ? `Last connected ${new Date(device.lastSeenAt).toLocaleString()}` : "Not connected yet"}</p></div><Button className="icon-button danger-icon" title={`Forget ${device.deviceName}`} onClick={() => forget(device.deviceId)}><Trash2 size={18} /></Button></article>)}</div>}
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
    <footer><span /><Button ref={confirmationButtonRef} className="secondary" onClick={cancelConfirmation}>Keep editing</Button><Button className={confirmation === "duplicate" ? "primary" : "primary danger"} disabled={busy} onClick={confirmAction}>{confirmationAction}</Button></footer>
  </section></div>;

  return <div className="modal-backdrop"><section ref={dialogRef} className="profile-dialog" role="dialog" aria-modal="true" aria-labelledby="profile-title" tabIndex={-1} onKeyDown={trapFocus}>
    <header><div><h2 id="profile-title">{profile.builtIn ? profile.name : "Edit switch profile"}</h2><p>Map each physical switch to a desktop action.</p></div><Button className="icon-button" title="Close" onClick={requestClose}><X size={18} /></Button></header>
    {operationError && <div className="dialog-error" role="alert">{operationError}</div>}
    <label className="field"><span>Profile name</span><Input data-error-key="name" value={draft.name} maxLength={50} disabled={profile.builtIn} aria-invalid={Boolean(errors.name)} aria-describedby={errors.name ? "profile-name-error" : undefined} onChange={(event) => setDraft({ ...draft, name: event.target.value })} />{errors.name && <span className="field-error" id="profile-name-error">{errors.name}</span>}</label>
    <div className="binding-list">{draft.bindings.map((binding, index) => <div className="binding-row" key={binding.switchId} data-invalid={Boolean(errors[`binding-${binding.switchId}`])}>
      <strong>Switch {binding.switchId}</strong>
      <Select data-error-key={`binding-${binding.switchId}`} aria-label={`Switch ${binding.switchId} action`} aria-invalid={Boolean(errors[`binding-${binding.switchId}`])} aria-describedby={errors[`binding-${binding.switchId}`] ? `binding-${binding.switchId}-error` : undefined} value={binding.type} disabled={profile.builtIn} onChange={(event) => setBinding(index, event.target.value as typeof binding.type)}>
        <option value="none">No action</option><option value="key">Key</option><option value="shortcut">Shortcut</option><option value="mouseButton">Hold mouse button</option><option value="mouseClick">Mouse click</option><option value="scroll">Scroll</option><option value="media">Media</option>
      </Select>
      {(binding.type === "key" || binding.type === "shortcut") && <Input className="key-recorder" aria-label={`Switch ${binding.switchId} key`} placeholder="Select, then press key" value={bindingLabel(binding)} readOnly disabled={profile.builtIn} onKeyDown={(event) => recordKey(index, binding, event)} />}
      {(binding.type === "mouseButton" || binding.type === "mouseClick") && <Select aria-label={`Switch ${binding.switchId} mouse button`} value={binding.value ?? "left"} disabled={profile.builtIn} onChange={(event) => setBinding(index, binding.type, event.target.value)}><option value="left">Left</option><option value="right">Right</option><option value="middle">Middle</option></Select>}
      {binding.type === "scroll" && <Select aria-label={`Switch ${binding.switchId} scroll direction`} value={binding.value ?? "down"} disabled={profile.builtIn} onChange={(event) => setBinding(index, binding.type, event.target.value)}><option value="up">Up</option><option value="down">Down</option><option value="left">Left</option><option value="right">Right</option></Select>}
      {binding.type === "media" && <Select aria-label={`Switch ${binding.switchId} media action`} value={binding.value ?? "playPause"} disabled={profile.builtIn} onChange={(event) => setBinding(index, binding.type, event.target.value)}><option value="playPause">Play / pause</option><option value="nextTrack">Next track</option><option value="previousTrack">Previous track</option><option value="volumeUp">Volume up</option><option value="volumeDown">Volume down</option><option value="mute">Mute</option></Select>}
      {errors[`binding-${binding.switchId}`] && <span className="field-error binding-error" id={`binding-${binding.switchId}-error`}>{errors[`binding-${binding.switchId}`]}</span>}
    </div>)}</div>
    <footer>{onDelete && <Button className="secondary danger" disabled={busy} onClick={() => setConfirmation("delete")}><Trash2 size={16} />Delete</Button>}<Button className="secondary" disabled={busy} onClick={() => dirty ? setConfirmation("duplicate") : onDuplicate()}><Copy size={16} />Duplicate</Button><span /><Button className="secondary" onClick={requestClose}>Cancel</Button>{!profile.builtIn && <Button className="primary" disabled={busy || Boolean(firstError)} onClick={() => void runSave()}><Save size={16} />Save profile</Button>}</footer>
  </section></div>;
}

function ProfilesView({ profiles, platform, saveProfile, deleteProfile, onDirtyChange, nativeExitRequest, onConfirmNativeExit, onCancelNativeExit, busy }: { profiles: SwitchProfile[]; platform: AppState["capabilities"]["platform"]; saveProfile: (profile: SwitchProfile) => Promise<void>; deleteProfile: (id: string) => Promise<void>; onDirtyChange: (dirty: boolean) => void; nativeExitRequest: ProfileExitAction | null; onConfirmNativeExit: () => void; onCancelNativeExit: () => void; busy: boolean }) {
  const [editing, setEditing] = useState<SwitchProfile | null>(null);
  const openerRef = useRef<HTMLButtonElement | null>(null);
  const closeEditor = () => { setEditing(null); requestAnimationFrame(() => (openerRef.current?.isConnected ? openerRef.current : document.querySelector<HTMLButtonElement>(".section-header button"))?.focus()); };
  const openEditor = (profile: SwitchProfile, opener: HTMLButtonElement) => { openerRef.current = opener; setEditing(profile); };
  return <div className="subview" data-section="profiles" data-setting="profiles"><header className="section-header"><div><h2>Switch Forwarding</h2><p>Profiles available to physical switch sessions</p></div><Button className="primary" onClick={(event) => openEditor(newProfile(), event.currentTarget)}><Plus size={16} />New profile</Button></header>
    <div className="profile-list">{profiles.map((profile) => <Button className="profile-row" key={profile.id} onClick={(event) => openEditor(profile, event.currentTarget)}><StatusIcon><SlidersHorizontal size={19} /></StatusIcon><div><h3>{profile.name}</h3><p>{profile.provider === "grid3" ? "Grid 3" : `${profile.bindings.filter((binding) => binding.type !== "none").length} mapped switches`}</p></div><span>{profile.builtIn ? "Built in" : "Custom"}</span><ChevronRight size={18} /></Button>)}</div>
    {platform === "macos" && <p className="capability-note">Grid 3 profiles are available on Windows only.</p>}
    {editing && <ProfileEditor key={editing.id} profile={editing} profiles={profiles} busy={busy} onDirtyChange={onDirtyChange} nativeExitRequest={nativeExitRequest} onConfirmNativeExit={() => { closeEditor(); onConfirmNativeExit(); }} onCancelNativeExit={onCancelNativeExit} onClose={closeEditor} onDuplicate={() => setEditing(duplicateProfile(editing, profiles))} onSave={async (profile) => { await saveProfile(profile); closeEditor(); }} onDelete={editing.builtIn || !profiles.some((profile) => profile.id === editing.id) ? null : async () => { await deleteProfile(editing.id); closeEditor(); }} />}
  </div>;
}

function UpdateBanner({ update, openUpdates }: { update: UpdateState; openUpdates: () => void }) {
  if (update.status !== "available" && update.status !== "downloading" && update.status !== "readyToInstall") return null;
  const message = update.status === "available"
    ? `Switchify PC ${update.version} is available.`
    : update.status === "downloading"
      ? `Downloading Switchify PC ${update.version} — ${updateProgress(update)}`
      : `Switchify PC ${update.version} is ready to install.`;
  return <section className="update-banner" role="status" aria-label="Application update">
    <Download size={18} aria-hidden="true" />
    <p>{message}</p>
    <Button className="text-button" type="button" onClick={openUpdates}>{update.status === "downloading" ? "View progress" : "View update"}</Button>
  </section>;
}

function SupportView({ state, switches, busy, perform, openSetup, openUpdates, ctx, jump }: { state: AppState; switches: SwitchController; busy: boolean; perform: (operation: () => Promise<AppState>) => void; openSetup: () => void; openUpdates: () => void; ctx: FindContext; jump: Jump }) {
  return <div className="view"><PageHead title="Help" blurb={<>Set up Switchify PC and fix common problems. The full guide is at <a href="https://pc.theswitchify.app" target="_blank" rel="noreferrer">pc.theswitchify.app</a>.</>} />
    <SectionStrip page="support" ctx={ctx} jump={jump} />
    <section className="subview" data-section="setup" aria-labelledby="support-setup-heading"><header className="section-header" data-setting="setup-guide"><div><h2 id="support-setup-heading">Setup</h2><p>The three things a working setup needs.</p></div><Button className="primary setup-launch" onClick={openSetup}><Wrench size={16} />Open setup guide</Button></header><div className="task-list" aria-label="Setup status">
      <article data-setting="input-access"><StatusIcon ok={state.accessibility === "granted"}><Accessibility size={19} /></StatusIcon><div><h3>Input access</h3><AccessibilityCopy state={state} detailed /><Demonstration kind="access" /></div>{state.accessibility === "required" && <Button className="secondary" disabled={busy} onClick={() => perform(() => api.checkAccessibility(true))}>Open Accessibility Settings</Button>}</article>
      <article><StatusIcon><Keyboard size={19} /></StatusIcon><div><h3>Local switches</h3><p>{switches.state?.settings.bindings.length ?? 0} configured. Open Switches to assign Select, then open Scanning to adjust movement.</p></div><Button className="secondary" onClick={() => jump({ page: "switches", section: "switches" })}>Open Switches</Button></article>
      <article><StatusIcon><Bluetooth size={19} /></StatusIcon><div><h3>Optional mobile connection</h3><p>{bluetoothLabels[state.bluetooth]}. Open Mobile for pairing and forwarding profiles.</p></div><Button className="secondary" onClick={() => jump({ page: "mobile", section: "pairing" })}>Open Mobile</Button></article>
    </div></section>
    <section className="subview" data-section="fix" aria-labelledby="support-fix-heading"><header className="section-header"><div><h2 id="support-fix-heading">Fix a problem</h2><p>Checks and tools for when something is not working.</p></div></header><div className="task-list" aria-label="Troubleshooting actions">
      <article data-setting="bluetooth"><StatusIcon><Bluetooth size={19} /></StatusIcon><div><h3>Bluetooth connection</h3><p>{bluetoothLabels[state.bluetooth]}</p></div><Button className="secondary" disabled={busy} onClick={() => perform(api.disconnectAll)}><Power size={16} />Disconnect</Button></article>
      <article><StatusIcon ok={state.accessibility === "granted"}><Accessibility size={19} /></StatusIcon><div><h3>Input access</h3><AccessibilityCopy state={state} detailed /><Demonstration kind="access" /></div>{state.accessibility === "required" ? <Button className="secondary" disabled={busy} onClick={() => perform(() => api.checkAccessibility(true))}>Open Accessibility Settings</Button> : <Button className="secondary" disabled={busy} onClick={() => perform(() => api.checkAccessibility(false))}><RefreshCw size={16} />Check input access</Button>}</article>
      <article data-setting="not-recognised"><StatusIcon><Keyboard size={19} /></StatusIcon><div><h3>Switch not recognized</h3><p>{interfaceModeAdvice} {reservedKeyAdvice(state.capabilities.platform)}</p><Demonstration kind="interface" /></div><Button className="secondary" onClick={() => jump({ page: "switches", section: "practice", setting: "practice" })}>Test switches</Button></article>
      <article><StatusIcon><RefreshCw size={19} /></StatusIcon><div><h3>Application update</h3><p>Switchify PC {state.version}</p></div><Button className="secondary" onClick={openUpdates}>View updates</Button></article>
      <article data-setting="diagnostics"><StatusIcon><Download size={19} /></StatusIcon><div><h3>Diagnostics</h3><p>Export sanitized health, capability, and recent event data</p></div><Button className="secondary" disabled={busy} onClick={() => perform(api.exportDiagnostics)}><Download size={16} />Export</Button></article>
      <article className="diagnostic-detail"><StatusIcon><Bluetooth size={19} /></StatusIcon><div><h3>Recent Bluetooth changes</h3><p>{state.diagnostics.recentBluetooth.length > 0 ? state.diagnostics.recentBluetooth.map((event) => event.status).join(", then ") : "No Bluetooth changes recorded yet"}</p></div></article>
      <article className="diagnostic-detail"><StatusIcon><Power size={19} /></StatusIcon><div><h3>Last disconnect</h3><p>{state.diagnostics.lastDisconnect ? `${state.diagnostics.lastDisconnect.detail ?? state.diagnostics.lastDisconnect.status}` : "No disconnect recorded yet"}</p></div></article>
      <article className="diagnostic-detail"><StatusIcon><CircleHelp size={19} /></StatusIcon><div><h3>Recent errors</h3><p>{state.diagnostics.recentErrors.length > 0 ? state.diagnostics.recentErrors.map((event) => event.detail ?? event.status).join(". ") : "No recent errors"}</p></div></article>
    </div></section>
    <AlsoSee page="support" ctx={ctx} jump={jump} />
  </div>;
}

function SetupGuide({ state, switches, suspended, busy, error, skip, finish, accessibility }: {
  state: AppState;
  busy: boolean;
  error: string | null;
  skip: () => Promise<void>;
  finish: (startWithSystem: boolean, shareDiagnostics: boolean) => Promise<void>;
  accessibility: () => Promise<void>;
  switches: SwitchController;
  suspended: boolean;
}) {
  const [step, setStep] = useState(0);
  const [startupChoice, setStartupChoice] = useState<boolean | null>(state.setup.completed ? state.settings.startWithSystem : null);
  const [diagnosticsChoice, setDiagnosticsChoice] = useState<boolean | null>(state.setup.completed && state.telemetry.consent !== "undecided" ? state.telemetry.consent === "enabled" : null);
  const dialogRef = useRef<HTMLElement>(null);
  const [switchDraft, setSwitchDraft] = useState(false);
  const capturing = switches.capturing || !!switches.state?.capture.active;
  const switchBusy = capturing || !!switches.pending || switches.unsaved || switchDraft;
  const titles = ["Input access", "Add your switch", "Scanning basics", "Start with system", "Anonymous diagnostics"];
  const canContinue = step === 1 ? !switchBusy && !!switches.state?.settings.bindings.length : step === 3 ? startupChoice !== null : step === 4 ? diagnosticsChoice !== null : true;
  const guidanceId = useId();
  const navigationGuidance = capturing
    ? "Finish capturing a switch or cancel capture before continuing."
    : switches.pending
      ? "Wait for your switch changes to finish saving before continuing."
      : switches.unsaved
        ? "Your switch changes have not saved. Retry saving before continuing."
        : switchDraft
          ? "Save or cancel the new switch before continuing."
          : busy
            ? "Please wait while setup saves your changes."
            : step === 1 && !switches.state?.settings.bindings.length
              ? "Add and save a local switch to enable Next. If you use only remote switches or want to configure switches later, choose Skip switch configuration to continue setup. Remote presets alone do not confirm a connected switch."
              : step === 3 && startupChoice === null
                ? "Choose Start with system or Start manually to enable Next."
                : step === 4 && diagnosticsChoice === null
                  ? "Choose whether to share diagnostics to enable Finish."
                  : null;

  useEffect(() => { if (!suspended && !capturing) dialogRef.current?.focus(); }, [step, suspended]);


  const trapFocus = (event: React.KeyboardEvent<HTMLElement>) => {
    if (event.key !== "Tab" || capturing || suspended) return;
    const controls = [...(dialogRef.current?.querySelectorAll<HTMLElement>('button:not(:disabled):not([tabindex="-1"]), a[href], input:not(:disabled), select:not(:disabled)') ?? [])];
    if (controls.length === 0) return;
    const first = controls[0]; const last = controls.at(-1)!;
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
  };

  return <div className="modal-backdrop setup-backdrop" hidden={suspended} inert={suspended}><section ref={dialogRef} className="setup-dialog" data-switch-step={step === 1} role={capturing ? undefined : "dialog"} aria-modal={capturing ? undefined : true} aria-labelledby="setup-title" tabIndex={-1} onKeyDown={trapFocus}>
    <header><div><span>Setup guide</span><h2 id="setup-title">{titles[step]}</h2></div><strong aria-label={`Step ${step + 1} of 5`}>{step + 1} / 5</strong></header>
    <div className="setup-progress" aria-hidden="true">{titles.map((title, index) => <span key={title} data-active={index <= step} />)}</div>
    {error && <div className="dialog-error" role="alert">{error}</div>}
    <div className="setup-content">
      {step === 0 && <div className="setup-statuses">
        <article><StatusIcon ok={state.accessibility === "granted"}><Accessibility size={19} /></StatusIcon><div><h3>Input access</h3><AccessibilityCopy state={state} detailed /><Demonstration kind="access" /></div>{state.accessibility === "required" && <Button className="secondary" disabled={busy} onClick={() => void accessibility()}>Open Accessibility Settings</Button>}</article>
      </div>}
      {step === 1 && <SwitchesSection mobileConnected={state.bluetooth === "connected"} platform={state.capabilities.platform} controller={switches} onDraftChange={setSwitchDraft} suspended={suspended} />}
      {step === 2 && <div><h3>Use your switches</h3><p>Press and release Select to open Home. Choose Point to pick a place on the screen and an action, Mouse to move a visible pointer ring with mouse controls, or Keyboard to type. Apps and windows brings another app forward, so you never need to focus an app first. After an action, Select carries on in the same mode.</p><p>Automatic scanning moves the highlight for you. Use Auto scan rate for grid and panel timing, and Line speed for point scanning lines. Holding a switch pauses the scan highlight; release runs the action shown. Choose whether repeating Mouse movement and scrolling stop on switch press or release under Scanning, Mouse pointer.</p><p>Escape disables switch control. Assign Stop scanning or Pause / resume in Switches. Holding any switch for {switches.state ? switches.state.escapeHoldMs / 1000 : 4}s disables switch control.</p><p>Open Scanning after setup to adjust movement and switch between Point and Mouse. Mobile connection is optional.</p><Demonstration kind="grid" /><Demonstration kind="line" /><Demonstration kind="hold" /></div>}
      {step === 3 && <div><h3>Choose startup behavior</h3><p>Switchify can start quietly when you sign in, ready for your switches.</p><div className="setup-choices" role="group" aria-label="Start with system choice"><Button className="secondary" aria-pressed={startupChoice === true} onClick={() => setStartupChoice(true)}>Start with system</Button><Button className="secondary" aria-pressed={startupChoice === false} onClick={() => setStartupChoice(false)}>Start manually</Button></div><Demonstration kind="startup" /></div>}
      {step === 4 && <div><h3>Choose whether to share diagnostics</h3><p>Optional anonymous app health and sanitized errors help improve Switchify. Typed text, commands, pairing secrets, device names, and full paths are never included.</p><div className="setup-choices" role="group" aria-label="Anonymous diagnostics choice"><Button className="secondary" disabled={!state.telemetry.available} aria-pressed={diagnosticsChoice === true} onClick={() => setDiagnosticsChoice(true)}>Share diagnostics</Button><Button className="secondary" aria-pressed={diagnosticsChoice === false} onClick={() => setDiagnosticsChoice(false)}>Don’t share</Button></div><a className="setup-privacy" href="https://www.theswitchify.app/privacy" target="_blank" rel="noreferrer">Privacy policy</a></div>}
    </div>
    <p id={guidanceId} className="setup-navigation-guidance" role="status">{navigationGuidance}</p>
    <footer><Button className="text-button" disabled={busy || switchBusy} onClick={() => void skip()}>Skip setup for now</Button>{step === 1 && <Button className="secondary" disabled={busy || switchBusy} onClick={() => setStep(2)}>Skip switch configuration</Button>}<span /><Button className="secondary" disabled={busy || switchBusy || step === 0} onClick={() => setStep((current) => current - 1)}>Back</Button><Button className="primary" aria-describedby={navigationGuidance ? guidanceId : undefined} disabled={busy || switchBusy || !canContinue} onClick={() => step === 4 ? void finish(startupChoice!, diagnosticsChoice!) : setStep((current) => current + 1)}>{step === 4 ? "Finish" : "Next"}</Button></footer>
  </section></div>;
}

function PairingDialog({ requests, connectedDeviceName, busy, error, approve, reject }: {
  requests: PendingPairing[];
  error: string | null;
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
    <header><Smartphone size={26} /><div><h2 id="pairing-title">Pairing requests</h2><p>Confirm each code matches Switchify for mobile.</p>{connectedDeviceName && <p className="pairing-connection">Connected to {connectedDeviceName}</p>}</div><span>{requests.length}</span></header>
    {error && <p className="dialog-error" role="alert">{error}</p>}
    <div className="pairing-list" aria-label="Pending pairing requests">
      {requests.map((request, index) => {
        const titleId = `pairing-request-${index}`;
        const actionDescription = `${request.deviceName}, code ${request.verificationCode}`;
        return <article key={request.requestId} aria-labelledby={titleId}>
          <div><h3 id={titleId}>{request.deviceName}</h3><p>Verification code</p></div>
          <output aria-label={`Verification code for ${request.deviceName}`}>{request.verificationCode}</output>
          <div className="pairing-actions"><Button className="secondary danger" disabled={busy} aria-label={`Reject pairing request from ${actionDescription}`} onClick={() => run(request.requestId, reject)}>Reject</Button><Button className="primary" disabled={busy} aria-label={`Accept pairing request from ${actionDescription}`} onClick={() => run(request.requestId, approve)}>Accept</Button></div>
        </article>;
      })}
    </div>
  </section></div>;
}

export function App() {
  const switches=useSwitches();
  const scanning=useScanning();
  const [state, setState] = useState<AppState | null>(null);
  // The one download-progress listener, owned here so the Updates section shows
  // the latest sample even when it mounts mid-download.
  const liveUpdater = useUpdateProgress(state?.updater);
  const pairingOpen = !!state?.pendingPairings.length;
  const captureActive = switches.capturing || !!switches.state?.capture.active;
  const cancelCapture = useRef(switches.cancelCapture);
  cancelCapture.current = switches.cancelCapture;
  useEffect(() => { if (pairingOpen && captureActive) void cancelCapture.current(); }, [pairingOpen, captureActive]);
  const [view, setView] = useState<View>("home");
  const customPanels = usePanels();
  const viewRef = useRef<View>("home");
  const jumpRef = useRef<Jump>(() => undefined);
  const [profiles, setProfiles] = useState<SwitchProfile[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [busy, setBusy] = useState(false);
  const [focusUpdates, setFocusUpdates] = useState(false);
  const [account, setAccount] = useState<AccountView | null>(null);
  const [query, setQuery] = useState("");
  const [arrival, setArrival] = useState<Target | null>(null);
  const [requestedArea, setRequestedArea] = useState<ScanArea | "shared" | null>(null);
  const rootRef = useRef<HTMLDivElement>(null);
  const findRef = useRef<HTMLInputElement>(null);
  const ctx = useMemo<FindContext | null>(() => state ? {
    state, settings, config: scanning.state ? scanning.config : null, scan: scanning.state, bindings: switches.state?.settings.bindings ?? null, holdIntervalMs: switches.state?.settings.holdIntervalMs ?? null,
    panels: customPanels.panels, builtins: customPanels.builtins, profiles: view === "mobile" ? profiles : null, account,
  } : null, [state, settings, scanning.config, scanning.state, switches.state, customPanels.panels, customPanels.builtins, profiles, account, view]);
  const recent = useRecent(ctx);
  // --- Update failures away from the Updates section --------------------------
  // Failed and cancelled are the updater states with something to act on that
  // the global banner deliberately leaves out: the scheduled check fails for
  // every offline user, so showing these app-wide would nag. They are surfaced
  // within Settings instead, whose Updates panel is where their status lives:
  // a marker on that tab, and a live region spoken from the other tabs. All of
  // it is owned here so it survives SettingsView unmounting between views, and
  // so the live region exists long before any text arrives in it.
  //
  // "The same failure" is the state plus the context before the backend's
  // `context: error` colon (update_failure in src-tauri/src/lib.rs): the error
  // half is transport text that can change wording from one scheduled check to
  // the next without anything the user could act on having changed.
  const settledFailure = useMemo(() => {
    if (!state || !updateStanding(state.updater.status)) return null;
    const text = updateDescription(state.updater);
    return { text, status: state.updater.status as "failed" | "cancelled", key: `${state.updater.status}:${text.split(": ")[0]}` };
  }, [state?.updater.status, state?.updater.error]);
  // The scheduled check flips a standing failure through "checking" and back.
  // Hold the last settled one through it so nothing blinks or repeats. A check
  // the user asked for is not held: whatever it returns is news.
  const [userCheck, setUserCheck] = useState(false);
  const inFlight = state ? updateInFlight(state.updater.status) && !userCheck : false;
  const [heldFailure, setHeldFailure] = useState(settledFailure);
  if (!inFlight && heldFailure !== settledFailure) setHeldFailure(settledFailure);
  const updateFailure = inFlight ? heldFailure : settledFailure;
  // Whether the Settings page, which holds the Updates section, is shown.
  const [updatesShown, setUpdatesShown] = useState(false);
  const announcedUpdateFailure = useRef<{ key: string; text: string } | null>(null);
  const [updateNotice, setUpdateNotice] = useState("");
  useEffect(() => {
    if (!updateFailure) { announcedUpdateFailure.current = null; setUpdateNotice(""); return; }
    if (updatesShown) { announcedUpdateFailure.current = { key: updateFailure.key, text: updateFailure.text }; setUpdateNotice(""); return; }
    // Find's results replace the page without changing the view, so they are
    // the one time the Updates section is off screen while on Settings.
    if (view !== "settings" || query) return;
    const announced = announcedUpdateFailure.current;
    if (updateFailure.key !== announced?.key) {
      announcedUpdateFailure.current = { key: updateFailure.key, text: updateFailure.text };
      setUpdateNotice(updateFailure.status === "failed" ? `${updateFailure.text} Open Updates in Settings to retry.` : updateFailure.text);
    } else if (announced && updateFailure.text !== announced.text) {
      // The same failure in different words is not worth an interruption, but
      // stale text must not sit there contradicting the tab's description.
      announcedUpdateFailure.current = { key: updateFailure.key, text: updateFailure.text };
      setUpdateNotice("");
    }
  }, [updateFailure, updatesShown, view, query]);
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
  const panelEditorDirty = useRef(false);
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

  const runUpdate = async (action: UpdateAction) => {
    setError(null);
    if (action === "check") setUserCheck(true);
    try {
      const operation = action === "check" ? api.checkForUpdates : action === "download" ? api.downloadUpdate : api.installUpdate;
      syncState(await operation());
    } catch (reason) { setError(String(reason)); }
    finally { if (action === "check") setUserCheck(false); }
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
    let active = true;
    // Read-only: loading the account for the index must never prompt for the keychain.
    void api.accountStatus().then((view) => { if (active) setAccount(view); }).catch(() => undefined);
    const stop = api.onAccount((view) => setAccount(view));
    return () => { active = false; void stop.then((unlisten) => unlisten()); };
  }, []);

  useEffect(() => {
    let unlisten: () => void = () => {};
    let disposed = false;
    void api.onNavigateRequested((target: NavigationTarget) => {
      if (disposed) return;
      if (target === "profiles") jumpRef.current({ page: "mobile", section: "profiles" });
      else selectView(target);
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
      if (!profileEditorDirty.current && !panelEditorDirty.current) return;
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", preventUnsavedUnload);
    return () => window.removeEventListener("beforeunload", preventUnsavedUnload);
  }, []);

  useEffect(() => { if (view === "mobile") void api.listProfiles().then(setProfiles).catch((reason) => setError(String(reason))); }, [view]);
  // Settings sync can change profiles from another computer; refresh the list
  // unless an edit is in progress, which a refresh would discard.
  useEffect(() => {
    if (view !== "mobile") return;
    const stop = api.onProfilesChanged(() => {
      if (!profileEditorDirty.current) void api.listProfiles().then(setProfiles).catch((reason) => setError(String(reason)));
    });
    return () => { void stop.then((unlisten) => unlisten()); };
  }, [view]);
  // Leaving an editor with unsaved changes asks first; a yes forgets them.
  const confirmDiscard = (): boolean => {
    if (profileEditorDirty.current && !window.confirm("Discard unsaved profile changes?")) return false;
    if (panelEditorDirty.current && !window.confirm("Discard unsaved panel changes?")) return false;
    profileEditorDirty.current = false;
    panelEditorDirty.current = false;
    return true;
  };
  const selectView = (next: View): boolean => {
    if (next === viewRef.current) return true;
    if (!confirmDiscard()) return false;
    viewRef.current = next;
    setView(next);
    return true;
  };

  // Finding things: the Find tile's query, and the setting a jump is heading to.
  // Find's results replace the page, so the first letter typed asks the same
  // question leaving the page would; a refused question leaves the box empty.
  const changeQuery = (next: string) => {
    // Blank input is no query, so a stray space never replaces the page.
    if (!next.trim()) { setQuery(""); return; }
    if (!query && !confirmDiscard()) return;
    setQuery(next);
  };
  const jump: Jump = (target) => {
    setQuery("");
    if (!selectView(target.page)) return;
    setRequestedArea(target.area ?? (target.page === "scanning" ? "shared" : null));
    setArrival(target);
  };
  useArrival(rootRef, arrival, () => setArrival(null));
  // A new page starts at its top; a jump scrolls to its own target instead.
  useEffect(() => { if (!arrival && !focusUpdates) document.scrollingElement?.scrollTo?.({ top: 0 }); }, [view, query]);
  jumpRef.current = jump;
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      const target = event.target instanceof Element ? event.target : null;
      if (event.defaultPrevented || event.isComposing || target?.closest('[role="dialog"], [role="alertdialog"], .capture-dialog')) return;
      const shortcut = event.key.toLowerCase() === "k" && (event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey;
      const slash = event.key === "/" && !event.ctrlKey && !event.metaKey && !event.altKey;
      if (!shortcut && !slash) return;
      if (slash && target?.closest("input, select, textarea, [contenteditable]")) return;
      event.preventDefault();
      findRef.current?.focus();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
  const openUpdates = () => {
    // Find results replace the page, so leave them before heading to Updates.
    setQuery("");
    if (viewRef.current !== "settings") {
      if (!confirmDiscard()) return;
      viewRef.current = "settings";
      setView("settings");
    }
    setFocusUpdates(true);
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
  const modal = setupOpen || state.pendingPairings.length > 0;
  const current = ctx!;
  return <DemonstrationProvider platform={state.capabilities.platform} suspended={state.pendingPairings.length > 0 || switches.capturing || !!switches.state?.capture.active}><div ref={rootRef} className="app-shell" data-scan={scanning.config.scannerColor} data-thickness={scanning.config.scanPreferences?.thickness ?? "standard"}>
    <header className="app-bar" inert={modal}>
      <div className="brand"><img className="brand-mark" src={brandIconUrl} alt="" aria-hidden="true" /><div><strong>Switchify</strong><small>PC</small></div></div>
      <nav aria-label="Pages">{pages.map(([id, icon]) => <Button key={id} className="tile tile-nav" data-utility={id === "settings" || id === "support" ? id : undefined} data-active={view === id && !query} aria-current={view === id && !query ? "page" : undefined} onClick={() => { setQuery(""); selectView(id); }}><span className="tile-icon" aria-hidden="true">{icon}</span><span>{pageLabels[id]}</span></Button>)}</nav>
    </header>
    <DemonstrationProvider platform={state.capabilities.platform} suspended={modal || switches.capturing || !!switches.state?.capture.active}><main inert={modal}>
      <div className="workspace-toolbar"><span className="workspace-location">{query ? "Find settings" : pageLabels[view]}</span><FindTile query={query} onChange={changeQuery} inputRef={findRef} shortcut={state.capabilities.platform === "macos" ? "⌘ K" : "Ctrl K"} /></div>
      {state.licence?.configured && (state.licence.status !== "licensed" || (state.licence.offlineDaysRemaining != null && state.licence.offlineDaysRemaining <= 7)) && <div className="licence-banner-wrap">
        <div className="licence-banner" data-locked={state.licence.locked ?? ["expired", "unavailable", "lease_expired", "revoked", "deactivated"].includes(state.licence.status)}>
          <KeyRound size={24} aria-hidden="true" />
          <div role="status" aria-label="Licence">
            <p className="licence-banner-message">{state.licence.message}</p>
            <p className="licence-banner-hint">See Settings, Licence.</p>
          </div>
          {state.licence.status !== "licensed" && <BuyLicence />}
        </div>
      </div>}
      {error && <div className="error-banner" role="alert">{error}<Button onClick={() => setError(null)}>Dismiss</Button></div>}
      <UpdateBanner update={liveUpdater ?? state.updater} openUpdates={openUpdates} />
      <p id="settings-updates-notice" className="sr-only" aria-live={updateFailure ? updateLiveness(updateFailure.status) : "polite"} aria-atomic="true">{updateNotice}</p>
      {query ? <FindResults query={query} ctx={current} jump={jump} /> : <>
      {view === "home" && <HomeView state={state} switches={switches} scanning={scanning} navigate={selectView} ctx={current} jump={jump} recent={recent} onDisconnect={() => void perform(api.disconnectAll)} onAccessibility={() => void perform(() => api.checkAccessibility(true))} onReleaseSwitchKeys={(released) => void perform(() => api.setSwitchKeysReleased(released))} />}
      {view === "switches" && <div className="view"><PageHead title="Switches" blurb="The switches you use to control this computer." /><SectionStrip page="switches" ctx={current} jump={jump} /><SwitchesSection mobileConnected={state.bluetooth === "connected"} platform={state.capabilities.platform} controller={switches} suspended={pairingOpen} /><AlsoSee page="switches" ctx={current} jump={jump} /></div>}
      {view === "scanning" && <div className="view"><PageHead title="Scanning" blurb="How the highlight moves and looks, and the pointer it drives." /><SectionStrip page="scanning" ctx={current} jump={jump} /><ScanningSection controller={scanning} settings={settings ?? undefined} onSettingsChange={changeSettings} requestedArea={requestedArea} onAreaOpened={() => setRequestedArea(null)} /><AlsoSee page="scanning" ctx={current} jump={jump} /></div>}
      {view === "panels" && <PanelsView controller={customPanels} theme={scanning.config.scanPreferences?.panelTheme} onDirtyChange={(dirty) => { panelEditorDirty.current = dirty; }} strip={<SectionStrip page="panels" ctx={current} jump={jump} />} alsoSee={<AlsoSee page="panels" ctx={current} jump={jump} />} />}
      {view === "mobile" && <div className="view"><PageHead title="Mobile" blurb="Control this computer from Switchify on a phone or tablet." /><SectionStrip page="mobile" ctx={current} jump={jump} />
        <MobileConnection state={state} onDisconnect={() => void perform(api.disconnectAll)} />
        <DevicesView state={state} forget={(id) => void perform(() => api.forgetDevice(id))} />
        <ProfilesView profiles={profiles} platform={state.capabilities.platform} busy={busy} saveProfile={saveProfile} deleteProfile={deleteProfile} onDirtyChange={(dirty) => { profileEditorDirty.current = dirty; }} nativeExitRequest={profileExitRequest} onConfirmNativeExit={confirmProfileExit} onCancelNativeExit={cancelProfileExit} />
        {state.capabilities.cursorOverlay && <div className="subview" data-section="cursor"><CursorSection settings={settings} update={(key, value) => changeSettings({ ...settings, [key]: value })} /></div>}
        <AlsoSee page="mobile" ctx={current} jump={jump} /></div>}
      {view === "settings" && <SettingsView state={state} updater={liveUpdater ?? state.updater} settings={settings} onChange={changeSettings} chooseTelemetry={(enabled) => void perform(() => api.setTelemetryConsent(enabled))} updateAction={(action) => void runUpdate(action)} cancelUpdate={() => void cancelUpdate()} busy={busy} focusUpdates={focusUpdates} onUpdatesFocused={() => setFocusUpdates(false)} onUpdatesShown={setUpdatesShown} ctx={current} jump={jump} />}
      {view === "support" && <SupportView state={state} switches={switches} busy={busy} perform={(operation) => void perform(operation)} openSetup={openSetup} openUpdates={openUpdates} ctx={current} jump={jump} />}
      </>}
    </main></DemonstrationProvider>
    {setupOpen && <SetupGuide state={state} switches={switches} suspended={state.pendingPairings.length > 0} busy={busy} error={error} skip={skipSetup} finish={finishSetup} accessibility={() => perform(() => api.checkAccessibility(true))} />}
    {state.pendingPairings.length > 0 && <PairingDialog error={error} requests={state.pendingPairings} connectedDeviceName={state.connectedDeviceName} busy={busy} reject={(requestId) => perform(() => api.rejectPairing(requestId))} approve={(requestId) => perform(() => api.approvePairing(requestId))} />}
  </div></DemonstrationProvider>;
}
