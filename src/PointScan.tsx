import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type PointScanConfig = {
  mode: "line" | "grid"; automatic: boolean; speed: number; gridSize: number; blockIntervalMs: number;
  selectKey: string; nextKey: string; backKey: string; pauseKey: string;
};
export type PointScanState = { config: PointScanConfig; enabled: boolean; phase: "idle" | "row" | "cell" | "x" | "y"; paused: boolean; message: string; supported: boolean };
export const defaultPointScanConfig: PointScanConfig = { mode: "line", automatic: true, speed: 2, gridSize: 4, blockIntervalMs: 1000, selectKey: "Space", nextKey: "Enter", backKey: "Backspace", pauseKey: "F8" };
const keys = ["Space", "Enter", "Backspace", "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight", ...Array.from({ length: 24 }, (_, i) => `F${i + 1}`)];
const phases = { idle: "Ready to begin", row: "Choose a row", cell: "Choose a cell", x: "Choose the horizontal position", y: "Choose the vertical position" };

export function PointScan() {
  const [state, setState] = useState<PointScanState | null>(null);
  const [config, setConfig] = useState(defaultPointScanConfig);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    const receive = (next: PointScanState) => { if (alive) { setState(next); setConfig(next.config); } };
    if (!("__TAURI_INTERNALS__" in window)) {
      setState({ config: defaultPointScanConfig, enabled: false, phase: "idle", paused: false, supported: false, message: "Open the desktop app to use point scan." });
      return;
    }
    void listen<PointScanState>("point-scan-changed", (event) => receive(event.payload)).then((stop) => {
      if (alive) unlisten = stop; else stop();
      return invoke<PointScanState>("get_point_scan");
    }).then(receive).catch((reason) => { if (alive) setError(String(reason)); });
    return () => { alive = false; unlisten?.(); };
  }, []);
  const apply = async (enabled: boolean) => {
    setBusy(true); setError(null);
    try { const next = await invoke<PointScanState>("configure_point_scan", { config, enabled }); setState(next); setConfig(next.config); }
    catch (reason) { setError(String(reason)); }
    finally { setBusy(false); }
  };
  const update = <K extends keyof PointScanConfig>(key: K, value: PointScanConfig[K]) => setConfig({ ...config, [key]: value });
  const invalidKeys = new Set([config.selectKey, config.nextKey, config.backKey, config.pauseKey]).size !== 4;
  return <div className="view point-scan-view">
    <header className="page-header"><div><h1>Point scan</h1><p>Select a point using switches connected to this computer.</p></div></header>
    <section className="settings-section">
      <h2>Local switch control</h2>
      <p>Enable point scan, focus the application you want to use, then press Select. The display under the pointer is scanned. Select the X position, then the Y position to click once.</p>
      <p>Switch keys are reserved while enabled. Escape stops scanning and releases them. Disconnect Android before enabling local point scan.</p>
      <p role="status">{state?.enabled ? `${state.paused ? "Paused. " : ""}${phases[state.phase]}.` : state?.message ?? "Loading point scan..."}</p>
      {error && <p role="alert">{error}</p>}
      {invalidKeys && <p role="alert">Each switch action needs a different key.</p>}
      <div className="button-row">
        <button type="button" disabled={busy || !state?.supported || invalidKeys} onClick={() => void apply(!state?.enabled)}>{state?.enabled ? "Disable point scan" : "Enable point scan"}</button>
        <button type="button" className="secondary" disabled={busy || !state?.supported || state.enabled || invalidKeys} onClick={() => void apply(false)}>Save settings</button>
      </div>
    </section>
    <fieldset disabled={busy || !!state?.enabled} className="point-scan-options">
      <legend>Scanning settings</legend>
      <label>Mode<select value={config.mode} onChange={(e) => update("mode", e.target.value as PointScanConfig["mode"])}><option value="line">Line only</option><option value="grid">Grid then line</option></select></label>
      <label>Movement<select value={String(config.automatic)} onChange={(e) => update("automatic", e.target.value === "true")}><option value="true">Automatic</option><option value="false">Manual</option></select></label>
      <label>Line speed<select value={config.speed} onChange={(e) => update("speed", Number(e.target.value))}>{["Very slow", "Slow", "Medium", "Fast", "Very fast"].map((label, value) => <option key={label} value={value}>{label}</option>)}</select></label>
      {config.mode === "grid" && <>
        <label>Grid size<select value={config.gridSize} onChange={(e) => update("gridSize", Number(e.target.value))}>{Array.from({ length: 9 }, (_, i) => i + 2).map((n) => <option key={n} value={n}>{n} × {n}</option>)}</select></label>
        <label>Grid interval<select value={config.blockIntervalMs} onChange={(e) => update("blockIntervalMs", Number(e.target.value))}>{[250, 500, 750, 1000, 1500, 2000, 3000, 4000, 5000].map((n) => <option key={n} value={n}>{n / 1000} seconds</option>)}</select></label>
      </>}
      {([["selectKey", "Select switch"], ["nextKey", "Forward switch"], ["backKey", "Backward switch"], ["pauseKey", "Pause / resume switch"]] as const).map(([key, label]) => <label key={key}>{label}<select value={config[key]} onChange={(e) => update(key, e.target.value)}>{keys.map((value) => <option key={value}>{value}</option>)}</select></label>)}
    </fieldset>
    <p>Select takes effect on release. Holding Select freezes scanning. Forward and Backward step once and set direction. After clicking, press Select to start again. Settings are saved, but scanning stays off when the app restarts.</p>
  </div>;
}
