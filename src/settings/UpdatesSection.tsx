import { Download, RefreshCw, X } from "lucide-react";
import type { UpdateState } from "../types";

export type UpdateAction = "check" | "download" | "install";

export function updateProgress(update: UpdateState) {
  if (update.totalBytes && update.totalBytes > 0) {
    return `${Math.min(100, Math.round(update.downloadedBytes * 100 / update.totalBytes))}%`;
  }
  return `${update.downloadedBytes.toLocaleString()} bytes`;
}

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
    case "failed": return update.error ?? "The update operation failed.";
    case "cancelled": return "Download cancelled. You can retry when ready.";
  }
}

export function UpdateControls({ update, run, cancel }: { update: UpdateState; run: (action: UpdateAction) => void; cancel: () => void }) {
  const action = update.status === "available" || update.status === "cancelled" ? "download"
    : update.status === "readyToInstall" ? "install"
      : update.status === "failed" ? update.retryAction
        : update.status === "idle" || update.status === "current" || update.status === "unconfigured" ? "check" : null;
  const label = update.status === "failed" ? "Retry"
    : action === "download" ? (update.status === "cancelled" ? "Retry download" : "Download")
      : action === "install" ? "Install and restart" : "Check for updates";
  return <div className="update-controls">
    <p role={update.status === "failed" ? "alert" : "status"}>{updateDescription(update)}</p>
    {update.status === "downloading" && <>
      <progress aria-label="Update download progress" value={update.downloadedBytes} max={update.totalBytes ?? undefined} />
      <span>{updateProgress(update)}</span>
    </>}
    <div>{action && <button className="secondary" type="button" onClick={() => run(action)}>{action === "download" && <Download size={16} />}{action === "check" && <RefreshCw size={16} />}{label}</button>}{update.status === "downloading" && <button className="secondary" type="button" onClick={cancel}><X size={16} />Cancel</button>}{(update.status === "checking" || update.status === "applying") && <button className="secondary" type="button" disabled><RefreshCw className="spin" size={16} />{update.status === "checking" ? "Checking" : "Installing"}</button>}</div>
  </div>;
}
