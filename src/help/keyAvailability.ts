import type { DemoPlatform } from "./demonstrations";

// Why a saved switch key cannot be used on this computer. Keys that a switch
// interface can send but this platform cannot capture: Windows reserves F12
// for debuggers, and macOS has no key codes above F20.
export function unavailableKeyReason(key: string, platform?: DemoPlatform) {
  const f = /^F(\d+)$/.exec(key);
  const number = f ? Number(f[1]) : null;
  if (platform === "windows" && key === "F12") return "Windows reserves F12, so it cannot be a switch key here. Set your interface to another key.";
  if (platform === "macos" && number !== null && number > 20) return `macOS has no ${key} key, so it cannot be a switch key here. Set your interface to F20 or lower.`;
  if (platform === "linux") return "Switch keys are not supported on this system yet.";
  return "This key is unavailable on this computer. Learn another key.";
}

// Told before learning, so a press that produces nothing is explained.
export function keysNotLearned(platform?: DemoPlatform) {
  if (platform === "windows") return "Windows reserves F12, so it will not be learned.";
  if (platform === "macos") return "F21 to F24 do not exist on macOS, so they will not be learned.";
  return null;
}

export const interfaceModeAdvice = "Set your switch interface to a keyboard or gamepad mode. Mouse-click modes cannot be learned as switches.";

// How a saved key reads on a badge: keyboard keys by name, controller
// buttons as "Gamepad 1" to "Gamepad 8".
export function switchKeyLabel(key: string) {
  const button = /^Gamepad([1-8])$/.exec(key);
  return button ? `Gamepad ${button[1]}` : key;
}
// The keys nothing else types: F13 and up, within what this platform can capture.
export function highKeyRange(platform?: DemoPlatform) {
  return platform === "macos" ? "F13 to F20" : "F13 or higher";
}
export function reservedKeyAdvice(platform?: DemoPlatform) {
  return `Assigned keys stay reserved while Switchify runs. If your interface can be programmed, choose ${highKeyRange(platform)} or a gamepad button so switch presses never clash with typing.`;
}
