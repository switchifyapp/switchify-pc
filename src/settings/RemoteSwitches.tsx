import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { actions, type SwitchAction } from "../scanning/useSwitches";
import { SettingGroup } from "./controls";
type Slot = { pressAction: SwitchAction | null; holdActions: SwitchAction[] };
type Config = { schemaVersion: number; revision: number; slots: Slot[] };
export function RemoteSwitches() {
  const [config, setConfig] = useState<Config | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    let alive = true;
    if (!("__TAURI_INTERNALS__" in window)) return;
    void invoke<Config>("get_remote_switches").then((value) => { if (alive) setConfig(value); }).catch((e) => { if (alive) setError(String(e)); });
    return () => { alive = false; };
  }, []);
  const update = (index: number, slot: Slot) => { if (config) setConfig({ ...config, slots: config.slots.map((s, i) => i === index ? slot : s) }); setSaved(false); };
  const save = async () => {
    if (!config) return;
    setBusy(true); setError(null); setSaved(false);
    try { setConfig(await invoke<Config>("save_remote_switches", { config })); setSaved(true); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  };
  return <SettingGroup title="Remote switches" description="Choose Switchify scanning in Remote’s Forwarding screen. Slot numbers match the switches shown there. These assignments use the shared scanning and hold timing settings.">
    <p className="setting-note">Remote owns scanning while forwarding. PC Escape stops it. Remote’s hold-to-stop and inactivity limits take priority over hold actions. Saving stops an active remote scan.</p>
    {error && <p role="alert">{error}</p>}
    {config && <fieldset disabled={busy}>
      {config.slots.map((slot, index) => <details key={index} className="switch-card"><summary>Remote switch {index + 1}: {slot.pressAction ? actions[slot.pressAction] : "Unassigned"}</summary>
        <label className="field"><span>Press and release</span><select aria-label={`Remote switch ${index + 1} action`} value={slot.pressAction ?? ""} onChange={(e) => update(index, { pressAction: e.target.value ? e.target.value as SwitchAction : null, holdActions: e.target.value ? slot.holdActions : [] })}>
          <option value="">Unassigned</option>{Object.entries(actions).map(([value, label]) => <option key={value} value={value}>{label}</option>)}
        </select></label>
        {slot.holdActions.map((action, hold) => <div className="field" key={hold}><label><span>Hold action {hold + 1}</span><select aria-label={`Remote switch ${index + 1} hold action ${hold + 1}`} value={action} onChange={(e) => update(index, { ...slot, holdActions: slot.holdActions.map((a, n) => n === hold ? e.target.value as SwitchAction : a) })}>{Object.entries(actions).map(([value, label]) => <option key={value} value={value}>{label}</option>)}</select></label>
          <button type="button" className="secondary" aria-label={`Remove remote switch ${index + 1} hold action ${hold + 1}`} onClick={() => update(index, { ...slot, holdActions: slot.holdActions.filter((_, n) => n !== hold) })}>Remove hold action</button>
        </div>)}
        <button type="button" className="secondary" disabled={!slot.pressAction || slot.holdActions.length >= 32} onClick={() => update(index, { ...slot, holdActions: [...slot.holdActions, "next"] })}>Add hold action for remote switch {index + 1}</button>
      </details>)}
      <button type="button" className="primary" onClick={() => void save()}>{busy ? "Saving…" : "Save remote switches"}</button>
    </fieldset>}
    {saved && <p role="status">Remote switches saved. Reload profiles in Remote before starting again.</p>}
  </SettingGroup>;
}
