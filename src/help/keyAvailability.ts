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

export const interfaceModeAdvice = "Set your switch interface to a keyboard mode. Mouse-click and gamepad modes cannot be learned as switches.";
export const reservedKeyAdvice = "Assigned keys stay reserved while Switchify runs. If your interface can be programmed, choose F13 or higher so switch presses never clash with typing.";
