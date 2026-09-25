import type { AppState } from "./types";
import type { PointScanState } from "./scanning/useScanning";

export type HomeTone = "ready" | "attention" | "neutral";
export type HomeStatus = { title: string; tone: HomeTone; message: string };

export type HomeStatusInput = {
  accessibility: AppState["accessibility"];
  bluetooth: AppState["bluetooth"];
  /** False until the switch settings have loaded. */
  switchesLoaded: boolean;
  hasSelect: boolean;
  error: string | null;
  scanning: PointScanState | null;
};

// One ordered list of cases, so the title, tone and message always describe
// the same state. Amber ("attention") is only for something a person can fix
// here; states outside their control, or still loading, stay neutral.
export function homeStatus({ accessibility, bluetooth, switchesLoaded, hasSelect, error, scanning }: HomeStatusInput): HomeStatus {
  if (error) return { title: "Switch control needs attention", tone: "attention", message: error };
  if (accessibility === "required") return { title: "Finish setting up", tone: "attention", message: "Allow input access to control this computer." };
  if (accessibility === "unavailable") return { title: "Input access unavailable", tone: "neutral", message: "Input access is unavailable on this system." };
  if (!scanning) return { title: "Switch control", tone: "neutral", message: "Loading switch control..." };
  if (!scanning.supported) return { title: "Switch control unavailable", tone: "neutral", message: scanning.message };
  if (scanning.remote) return { title: "Remote in control", tone: "neutral", message: "Remote controls scanning. Use the switches in Remote; PC Escape stops the session." };
  if (bluetooth === "connected") return { title: "Paused for mobile", tone: "neutral", message: "Local scanning is paused while a mobile device is connected." };
  if (!switchesLoaded) return { title: "Switch control", tone: "neutral", message: "Loading switch control..." };
  if (!hasSelect) return { title: "Finish setting up", tone: "attention", message: "Add a switch with the Select action to begin scanning." };
  if (scanning.paused) return { title: "Paused", tone: "neutral", message: "Scanning is paused. Use your Pause / resume switch to continue." };
  if (scanning.enabled) return { title: "Ready", tone: "ready", message: "Focus the application you want to use, then press and release your Select switch." };
  return { title: "Switch control", tone: "neutral", message: scanning.message };
}
