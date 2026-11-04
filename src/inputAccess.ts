import type { AppState } from "./types";

/** Accessibility setup is actionable only on a Mac without permission. */
export function needsInputAccess(state: AppState): boolean {
  return state.capabilities.platform === "macos" && state.accessibility === "required";
}
