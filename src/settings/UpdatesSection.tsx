import { Button } from "../ui/controls";
import { Download, RefreshCw, X } from "lucide-react";
import { useEffect, useState, type Ref } from "react";
import { api } from "../api";
import type { AppState, UpdateProgress, UpdateState, UpdateStatus } from "../types";
import { SettingGroup } from "./controls";

export type UpdateAction = "check" | "download" | "install";

// Download progress arrives on its own throttled event rather than in
// AppState, so only the components showing it re-render. AppState still
// carries the status transitions and the final size once ready.
export function useUpdateProgress(update: UpdateState): UpdateState {
  const downloading = update.status === "downloading";
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  // Stay subscribed while mounted so the first samples of a download are not
  // lost to a listener that is still registering.
  useEffect(() => {
    let disposed = false;
    let unlisten: () => void = () => {};
    void api.onUpdateProgress((next) => { if (!disposed) setProgress(next); }).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => { disposed = true; unlisten(); };
  }, []);
  // A finished, failed or cancelled download leaves no stale progress behind.
  useEffect(() => {
    if (!downloading) return;
    return () => setProgress(null);
  }, [downloading, update.version]);
  return downloading && progress ? { ...update, ...progress } : update;
}

export function updateProgress(update: UpdateState) {
  if (update.totalBytes && update.totalBytes > 0) {
    return `${Math.min(100, Math.round(update.downloadedBytes * 100 / update.totalBytes))}%`;
  }
  return `${update.downloadedBytes.toLocaleString()} bytes`;
}

// Backend failure text is `context: error` with no terminal punctuation.
const sentence = (text: string) => /[.!?]$/.test(text) ? text : `${text}.`;

export function updateDescription(update: UpdateState) {
  switch (update.status) {
    case "unconfigured": return "Updates are unavailable in this build because its signed feed is not configured.";
    case "idle": return "Automatic update checks are enabled.";
    case "checking": return "Checking for updates…";
    case "available": return `Switchify PC ${update.version} is available.`;
    case "downloading": return `Downloading Switchify PC ${update.version}…`;
    case "readyToInstall": return `Switchify PC ${update.version} is ready to install.`;
    case "applying": return `Installing Switchify PC ${update.version}…`;
    case "current": return "Switchify PC is up to date.";
    case "failed": return sentence(update.error ?? "The update operation failed.");
    case "cancelled": return "Download cancelled. You can retry when ready.";
  }
}

// Failures interrupt; everything else waits its turn. One rule for the panel's
// live region and the off-tab notice in SettingsView, so the two can never
// announce at different levels.
export function updateLiveness(status: UpdateStatus) {
  return status === "failed" ? "assertive" : "polite";
}
export function updateStatusRole(update: UpdateState) {
  return updateLiveness(update.status) === "assertive" ? "alert" : "status";
}

// The states with something left to act on, and the one the scheduled check
// passes through on its way back to them. An install in progress is neither:
// it is the user acting.
export function updateStanding(status: UpdateStatus) {
  return status === "failed" || status === "cancelled";
}
export function updateInFlight(status: UpdateStatus) {
  return status === "checking";
}

export function UpdateControls({ update: state, run, cancel }: { update: UpdateState; run: (action: UpdateAction) => void; cancel: () => void }) {
  const update = useUpdateProgress(state);
  const action = update.status === "available" || update.status === "cancelled" ? "download"
    : update.status === "readyToInstall" ? "install"
      : update.status === "failed" ? update.retryAction
        : update.status === "idle" || update.status === "current" || update.status === "unconfigured" ? "check" : null;
  const label = update.status === "failed" ? "Retry"
    : action === "download" ? (update.status === "cancelled" ? "Retry download" : "Download")
      : action === "install" ? "Install and restart" : "Check for updates";
  return <div className="update-controls">
    <p role={updateStatusRole(update)}>{updateDescription(update)}</p>
    {update.status === "downloading" && <>
      <progress aria-label="Update download progress" value={update.downloadedBytes} max={update.totalBytes ?? undefined} />
      <span>{updateProgress(update)}</span>
    </>}
    <div>{action && <Button className="secondary" type="button" onClick={() => run(action)}>{action === "download" && <Download size={16} />}{action === "check" && <RefreshCw size={16} />}{label}</Button>}{update.status === "downloading" && <Button className="secondary" type="button" onClick={cancel}><X size={16} />Cancel</Button>}{(update.status === "checking" || update.status === "applying") && <Button className="secondary" type="button" disabled><RefreshCw className="spin" size={16} />{update.status === "checking" ? "Checking" : "Installing"}</Button>}</div>
  </div>;
}

export function UpdatesSection({ state, run, cancel, sectionRef }: { state: AppState; run: (action: UpdateAction) => void; cancel: () => void; sectionRef: Ref<HTMLElement> }) {
  return <SettingGroup id="settings-updates" sectionRef={sectionRef} focusable title="Updates" description={`Switchify PC ${state.version}`}><UpdateControls update={state.updater} run={run} cancel={cancel} /></SettingGroup>;
}
