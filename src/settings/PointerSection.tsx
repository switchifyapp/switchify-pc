import type { AppSettings } from "../types";
import {
  OptionGroup, SettingGroup, Toggle, accelerationOptions, dwellDelayOptions, keyRepeatDelayOptions,
  movementValue, pointerSpeedOptions, pointerSpeedValues, repeatIntervalOptions, secondsOptions,
  type SettingsUpdate,
} from "./controls";

export function PointerSection({ settings, update }: { settings: AppSettings; update: SettingsUpdate }) {
  return <SettingGroup title="Pointer" description="Movement and visual feedback.">
      <fieldset className="pointer-speed"><legend>Pointer speed <strong>{settings.pointerScalePercent}%</strong></legend><div className="segmented compact five">
        {pointerSpeedOptions.map((value) => <button type="button" key={value} aria-label={`${value}% pointer speed`} aria-pressed={settings.pointerScalePercent === value} onClick={() => update("pointerScalePercent", value)}>{value}%</button>)}
      </div><label className="exact-speed"><span>Exact speed</span><select aria-label="Exact pointer speed" value={settings.pointerScalePercent} onChange={(event) => update("pointerScalePercent", Number(event.target.value))}>
        {pointerSpeedValues.map((value) => <option key={value} value={value}>{value}%</option>)}
      </select></label><div className="movement-values" aria-label="Pointer movement values">
        {([{"label":"Small","base":4.5},{"label":"Medium","base":12},{"label":"Large","base":26}] as const).map(({ label, base }) => <div key={label}><span>{label}</span><strong>{movementValue(base, settings.pointerScalePercent)}</strong></div>)}
      </div></fieldset>
      <div className="repeat-settings">
        <Toggle label="Repeat mouse movement" checked={settings.mouseRepeatEnabled} onChange={(value) => update("mouseRepeatEnabled", value)} />
        <div className="repeat-options" data-disabled={!settings.mouseRepeatEnabled}>
          <OptionGroup<number> legend="Movement interval" columns="four" disabled={!settings.mouseRepeatEnabled} options={secondsOptions(repeatIntervalOptions)} value={settings.moveRepeatIntervalMs} onChange={(next) => update("moveRepeatIntervalMs", next)} />
          <OptionGroup<number> legend="Movement acceleration" columns="four" disabled={!settings.mouseRepeatEnabled} options={accelerationOptions} value={settings.mouseRepeatAccelerationDurationMs} onChange={(next) => update("mouseRepeatAccelerationDurationMs", next)} />
          <OptionGroup<number> legend="Scroll interval" columns="four" disabled={!settings.mouseRepeatEnabled} options={secondsOptions(repeatIntervalOptions)} value={settings.scrollRepeatIntervalMs} onChange={(next) => update("scrollRepeatIntervalMs", next)} />
        </div>
      </div>
      <div className="repeat-settings">
        <Toggle label="Repeat held keys" checked={settings.keyRepeatEnabled} onChange={(value) => update("keyRepeatEnabled", value)} />
        <div className="repeat-options" data-disabled={!settings.keyRepeatEnabled}>
          <OptionGroup<number> legend="Delay before repeating" columns="four" disabled={!settings.keyRepeatEnabled} options={keyRepeatDelayOptions} value={settings.keyRepeatInitialDelayMs} onChange={(next) => update("keyRepeatInitialDelayMs", next)} />
          <OptionGroup<number> legend="Key interval" columns="four" disabled={!settings.keyRepeatEnabled} options={secondsOptions(repeatIntervalOptions)} value={settings.keyRepeatIntervalMs} onChange={(next) => update("keyRepeatIntervalMs", next)} />
          <p className="setting-note">Holding a navigation key on the remote repeats it, like holding a key on a keyboard. Applies to the arrow keys, Tab, Backspace, Delete, Page Up, and Page Down.</p>
        </div>
      </div>
      <div className="repeat-settings dwell-settings">
        <Toggle label="Dwell to click" checked={settings.dwellClickEnabled} onChange={(value) => update("dwellClickEnabled", value)} />
        <div className="repeat-options" data-disabled={!settings.dwellClickEnabled}>
          <OptionGroup<number> legend="Dwell delay" columns="five" disabled={!settings.dwellClickEnabled} options={secondsOptions(dwellDelayOptions)} value={settings.dwellClickDelayMs} onChange={(next) => update("dwellClickDelayMs", next)} />
          <p className="setting-note">After Android pointer movement stops, a countdown appears and performs one left click. Move again to rearm it.</p>
        </div>
      </div>
  </SettingGroup>;
}
