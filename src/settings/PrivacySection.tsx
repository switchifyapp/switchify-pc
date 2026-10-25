import { Button } from "../ui/controls";
import type { AppSettings, AppState } from "../types";
import { SettingGroup, Toggle, type SettingsUpdate } from "./controls";

// One affordance per consent state. Until a choice is recorded the only way
// to decide is the explicit Share / Don't share pair, which records consent
// either way; afterwards the toggle changes it (saving a changed flag records
// the new consent on the Rust side too). Showing both at once offered two
// paths to the same decision.
export function PrivacySection({ state, settings, update, chooseTelemetry, busy }: { state: AppState; settings: AppSettings; update: SettingsUpdate; chooseTelemetry: (enabled: boolean) => void; busy: boolean }) {
  const undecided = state.telemetry.consent === "undecided";
  return <SettingGroup title="Privacy" description="Optional anonymous app health and sanitized error reports. Never includes typed text, commands, pairing secrets, device names, or full paths.">{undecided ? <div className="privacy-choice" role="group" aria-label="Anonymous diagnostics choice"><Button className="secondary" type="button" disabled={busy || !state.telemetry.available} onClick={() => chooseTelemetry(true)}>Share diagnostics</Button><Button className="secondary" type="button" disabled={busy} onClick={() => chooseTelemetry(false)}>Don't share</Button></div> : <Toggle label="Share anonymous diagnostic data" disabled={!state.telemetry.available && !settings.shareDiagnostics} checked={settings.shareDiagnostics} onChange={(value) => update("shareDiagnostics", value)} />}<p className="setting-note">{state.telemetry.available ? undecided ? "No choice recorded yet. Nothing is sent unless you choose Share diagnostics." : state.telemetry.consent === "enabled" ? "Consent recorded. You can turn this off at any time to delete queued reports." : "Opted out. No diagnostic reports are stored or sent." : "Diagnostic reporting is unavailable in this build."} <a href="https://www.theswitchify.app/privacy" target="_blank" rel="noreferrer">Privacy policy</a></p></SettingGroup>;
}
