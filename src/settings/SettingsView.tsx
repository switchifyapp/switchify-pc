import { useEffect, useRef } from "react";
import { Settings } from "lucide-react";
import type { AppSettings, AppState } from "../types";
import {
  SettingGroup, Toggle, accelerationOptions, dwellDelayOptions, keyRepeatDelayOptions,
  movementValue, pointerSpeedOptions, pointerSpeedValues, repeatIntervalOptions,
} from "./controls";
import { UpdateControls, type UpdateAction } from "./UpdatesSection";

export function SettingsView({ state, settings, onChange, chooseTelemetry, updateAction, cancelUpdate, busy, focusUpdates, onUpdatesFocused }: { state: AppState; settings: AppSettings; onChange: (next: AppSettings) => void; chooseTelemetry: (enabled: boolean) => void; updateAction: (action: UpdateAction) => void; cancelUpdate: () => void; busy: boolean; focusUpdates: boolean; onUpdatesFocused: () => void }) {
  const updatesRef = useRef<HTMLElement>(null);
  useEffect(() => {
    if (!focusUpdates) return;
    updatesRef.current?.scrollIntoView?.({ block: "start" });
    updatesRef.current?.focus({ preventScroll: true });
    onUpdatesFocused();
  }, [focusUpdates, onUpdatesFocused]);
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
      <div className="repeat-settings">
        <Toggle label="Repeat held keys" checked={settings.keyRepeatEnabled} onChange={(value) => update("keyRepeatEnabled", value)} />
        <div className="repeat-options" data-disabled={!settings.keyRepeatEnabled}>
          <fieldset disabled={!settings.keyRepeatEnabled}><legend>Delay before repeating</legend><div className="segmented compact four">
            {keyRepeatDelayOptions.map(({ value, label }) => <button type="button" key={value} aria-pressed={settings.keyRepeatInitialDelayMs === value} onClick={() => update("keyRepeatInitialDelayMs", value)}>{label}</button>)}
          </div></fieldset>
          <fieldset disabled={!settings.keyRepeatEnabled}><legend>Key interval</legend><div className="segmented compact four">
            {repeatIntervalOptions.map((value) => <button type="button" key={value} aria-pressed={settings.keyRepeatIntervalMs === value} onClick={() => update("keyRepeatIntervalMs", value)}>{value / 1000}s</button>)}
          </div></fieldset>
          <p className="setting-note">Holding a navigation key on the remote repeats it, like holding a key on a keyboard. Applies to the arrow keys, Tab, Backspace, Delete, Page Up, and Page Down.</p>
        </div>
      </div>
      <div className="repeat-settings dwell-settings">
        <Toggle label="Dwell to click" checked={settings.dwellClickEnabled} onChange={(value) => update("dwellClickEnabled", value)} />
        <div className="repeat-options" data-disabled={!settings.dwellClickEnabled}>
          <fieldset disabled={!settings.dwellClickEnabled}><legend>Dwell delay</legend><div className="segmented compact five">
            {dwellDelayOptions.map((value) => <button type="button" key={value} aria-pressed={settings.dwellClickDelayMs === value} onClick={() => update("dwellClickDelayMs", value)}>{value / 1000}s</button>)}
          </div></fieldset>
          <p className="setting-note">After Android pointer movement stops, a countdown appears and performs one left click. Move again to rearm it.</p>
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
    <SettingGroup id="settings-updates" sectionRef={updatesRef} focusable title="Updates" description={`Switchify PC ${state.version}`}><UpdateControls update={state.updater} run={updateAction} cancel={cancelUpdate} /></SettingGroup>
  </div>;
}
