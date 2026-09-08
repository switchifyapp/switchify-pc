import type { AppSettings } from "../types";
import { SettingGroup, Toggle, type SettingsUpdate } from "./controls";

export function GeneralSection({ settings, update }: { settings: AppSettings; update: SettingsUpdate }) {
  return <SettingGroup title="General" description="System startup and background behavior."><Toggle label="Start with system" checked={settings.startWithSystem} onChange={(value) => update("startWithSystem", value)} /></SettingGroup>;
}
