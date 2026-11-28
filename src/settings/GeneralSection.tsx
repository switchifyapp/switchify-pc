import { Demonstration } from "../help/Demonstration";
import type { AppSettings, PlatformCapabilities } from "../types";
import { OptionGroup, SettingGroup, Toggle, type SettingsUpdate } from "./controls";
import { textSizeLabel, textSizePercents } from "./textSize";

const textSizeOptions = textSizePercents.map((value) => ({ value, label: textSizeLabel(value) }));

export function GeneralSection({ settings, update, platform }: { settings: AppSettings; update: SettingsUpdate; platform: PlatformCapabilities["platform"] }) {
  const summary = platform === "windows"
    ? "Match system follows the text size in Windows Settings. The scan panels and keyboard use Panel size instead."
    : `Match system uses 100%${platform === "macos" ? ", because macOS has no text size for all apps" : ""}. The scan panels and keyboard use Panel size instead.`;
  return <>
    <SettingGroup section="general" setting="startup" title="General" description="System startup and background behavior."><Toggle label="Start with system" checked={settings.startWithSystem} onChange={(value) => update("startWithSystem", value)} /><Demonstration kind="startup" /></SettingGroup>
    <SettingGroup section="general" setting="text-size" title="Text size" description="How big the text and controls are in this window.">
      <OptionGroup<number> legend="Text size" columns="three" disabled={false} options={textSizeOptions} value={settings.textSizePercent} onChange={(next) => update("textSizePercent", next)}
        note={{ summary, about: "text size" }} />
    </SettingGroup>
  </>;
}
