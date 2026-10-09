import { MoreOptions } from "../ui/controls";
import type { AppSettings } from "../types";
import {
  OptionGroup, SettingGroup, Toggle, dwellDelayOptions, keyRepeatDelayOptions,
  repeatIntervalOptions, secondsOptions, type SettingsUpdate,
} from "./controls";

export function InputSection({ settings, update }: { settings: AppSettings; update: SettingsUpdate }) {
  const repeating = settings.keyRepeatEnabled || settings.scanKeyRepeatEnabled;
  return <SettingGroup title="Input" description="Click after Remote pointer movement stops, or repeat navigation keys.">
    <div className="repeat-settings dwell-settings">
      <Toggle label="Click when I stop" checked={settings.dwellClickEnabled} onChange={(value) => update("dwellClickEnabled", value)} />
      <div className="repeat-options">
        <OptionGroup<number> legend="Wait before clicking" columns="five" disabled={!settings.dwellClickEnabled} options={secondsOptions(dwellDelayOptions)} value={settings.dwellClickDelayMs} onChange={(next) => update("dwellClickDelayMs", next)}
          note={{ summary: "After mobile pointer movement stops, a countdown appears and performs one left click. Move again to rearm it." }} />
      </div>
    </div>
    <div className="repeat-settings">
      <Toggle label="Repeat keys on the scanned keyboard" checked={settings.scanKeyRepeatEnabled} onChange={(value) => update("scanKeyRepeatEnabled", value)} />
      <p className="setting-note">Backspace, Delete, the arrow keys, Tab, Page Up and Page Down keep repeating after you choose them, until you press a switch. Choose whether that is on press or release under Mouse, Stop repeating.</p>
    </div>
    <MoreOptions><div className="repeat-settings">
      <Toggle label="Repeat held keys" checked={settings.keyRepeatEnabled} onChange={(value) => update("keyRepeatEnabled", value)} />
      <div className="repeat-options">
        <OptionGroup<number> legend="Delay before repeating" columns="four" disabled={!repeating} options={keyRepeatDelayOptions} value={settings.keyRepeatInitialDelayMs} onChange={(next) => update("keyRepeatInitialDelayMs", next)} />
        <OptionGroup<number> legend="Key interval" columns="four" disabled={!repeating} options={secondsOptions(repeatIntervalOptions)} value={settings.keyRepeatIntervalMs} onChange={(next) => update("keyRepeatIntervalMs", next)}
          note={{ about: "key repeat", summary: "Held navigation keys repeat, like on a keyboard.", detail: "Applies to the arrow keys, Tab, Backspace, Delete, Page Up, and Page Down. Keys repeating on the scanned keyboard use the same timing." }} />
      </div>
    </div></MoreOptions>
  </SettingGroup>;
}
