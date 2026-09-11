import type { AppState, PlatformCapabilities } from "./types";

// This only selects browser sample data. Native state comes from Rust.
export function browserPlatform(userAgent: string): PlatformCapabilities["platform"] {
  if (/Mac/i.test(userAgent)) return "macos";
  if (/Linux/i.test(userAgent) && !/Android/i.test(userAgent)) return "linux";
  return "windows";
}

export function linuxInputUnavailable(state: AppState): boolean {
  return state.capabilities.platform === "linux" && state.accessibility === "unavailable";
}

export const linuxInputDescription = "Input controls are not yet available in this Linux development build.";
export const linuxBluetoothDescription = "Bluetooth pairing is not yet available in this Linux development build.";

export function inputAccessAction(state: AppState): string {
  return state.capabilities.platform === "linux" ? "Check input access" : "Open Accessibility Settings";
}
