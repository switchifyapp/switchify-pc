import { MoreOptions } from "../ui/controls";
import type { AppSettings } from "../types";
import { SettingGroup, TimeStepper, Toggle, type SettingsUpdate } from "./controls";
import { dwellClickDelayRange, keyRepeatInitialDelayRange, keyRepeatIntervalRange } from "./timing";

export function InputSection({ settings, update }: { settings: AppSettings; update: SettingsUpdate }) {
  const repeating = settings.keyRepeatEnabled || settings.scanKeyRepeatEnabled;
  return <SettingGroup section="input" title="Input" description="Click after Remote pointer movement stops, or repeat navigation keys.">
    <div className="repeat-settings dwell-settings" data-setting="dwell">
      <Toggle label="Click when I stop" checked={settings.dwellClickEnabled} onChange={(value) => update("dwellClickEnabled", value)} />
      <div className="repeat-options">
        <TimeStepper label="Wait before clicking" range={dwellClickDelayRange} disabled={!settings.dwellClickEnabled} value={settings.dwellClickDelayMs} onChange={(next) => update("dwellClickDelayMs", next)}
          note={{ summary: "After mobile pointer movement stops, a countdown appears and performs one left click. Move again to rearm it. Set from 0.1 to 10 seconds." }} />
      </div>
    </div>
    <div className="repeat-settings" data-setting="key-repeat">
      <Toggle label="Repeat keys on the scanned keyboard" checked={settings.scanKeyRepeatEnabled} onChange={(value) => update("scanKeyRepeatEnabled", value)} />
      <p className="setting-note">Backspace, Delete, the arrow keys, Tab, Page Up and Page Down keep repeating after you choose them, until you press a switch. They use the key repeat timing under More options below. Choose whether a press or a release stops them under Scanning, Mouse pointer, Stop repeating.</p>
    </div>
    <MoreOptions><div className="repeat-settings">
      <Toggle label="Repeat held keys" checked={settings.keyRepeatEnabled} onChange={(value) => update("keyRepeatEnabled", value)} />
      <div className="repeat-options">
        <TimeStepper label="Delay before repeating" range={keyRepeatInitialDelayRange} zeroLabel="None" disabled={!repeating} value={settings.keyRepeatInitialDelayMs} onChange={(next) => update("keyRepeatInitialDelayMs", next)}
          note={{ summary: "How long a key waits before it starts repeating, from none to 2 seconds." }} />
        <TimeStepper label="Key interval" range={keyRepeatIntervalRange} disabled={!repeating} value={settings.keyRepeatIntervalMs} onChange={(next) => update("keyRepeatIntervalMs", next)}
          note={{ about: "key repeat", summary: "Held navigation keys repeat, like on a keyboard.", detail: "Applies to the arrow keys, Tab, Backspace, Delete, Page Up, and Page Down. Keys repeating on the scanned keyboard use the same timing." }} />
      </div>
    </div></MoreOptions>
  </SettingGroup>;
}
