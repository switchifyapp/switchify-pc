import type { AppSettings } from "../types";

export const changedSettingKeys = (previous: AppSettings, next: AppSettings) =>
  (Object.keys(next) as Array<keyof AppSettings>).filter((key) => previous[key] !== next[key]);

export function applyLocalSettings(base: AppSettings, local: AppSettings, keys: Set<keyof AppSettings>) {
  const merged = { ...base };
  for (const key of keys) Object.assign(merged, { [key]: local[key] });
  return merged;
}
