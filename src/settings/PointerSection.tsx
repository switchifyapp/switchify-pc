import { useEffect, useId, useState } from "react";
import type { AppSettings } from "../types";
import {
  Disclosure, OptionGroup, SettingGroup, Toggle, accelerationOptions, dwellDelayOptions, keyRepeatDelayOptions,
  movementValue, pointerSpeedOptions, pointerSpeedValues, repeatIntervalOptions, secondsOptions,
  type SettingsUpdate,
} from "./controls";

export function PointerSection({ settings, update }: { settings: AppSettings; update: SettingsUpdate }) {
  // The presets stop at 100%, so any value above that or off the 5/25/50/75
  // steps is only reachable through the exact-speed select. Reveal it whenever
  // such a value is active, so the control that produced it is never hidden.
  const isPreset = (pointerSpeedOptions as readonly number[]).includes(settings.pointerScalePercent);
  const [showExact, setShowExact] = useState(!isPreset);
  useEffect(() => { if (!isPreset) setShowExact(true); }, [isPreset]);
  const exactSpeedId = useId();
  return <SettingGroup title="Pointer" description="Movement and visual feedback.">
      <fieldset className="pointer-speed"><legend>Pointer speed <strong>{settings.pointerScalePercent}%</strong></legend><div className="segmented compact five">
        {pointerSpeedOptions.map((value) => <button type="button" key={value} aria-label={`${value}% pointer speed`} aria-pressed={settings.pointerScalePercent === value} onClick={() => update("pointerScalePercent", value)}>{value}%</button>)}
      </div><Disclosure label={showExact ? "Hide exact speed" : "Set an exact speed"} expanded={showExact} onToggle={() => setShowExact(!showExact)} controls={exactSpeedId}>
        <div id={exactSpeedId}>
          <label className="exact-speed"><span>Exact speed</span><select aria-label="Exact pointer speed" value={settings.pointerScalePercent} onChange={(event) => update("pointerScalePercent", Number(event.target.value))}>
            {pointerSpeedValues.map((value) => <option key={value} value={value}>{value}%</option>)}
          </select></label>
          <div className="movement-values" aria-label="Pointer movement values">
            {([{"label":"Small","base":4.5},{"label":"Medium","base":12},{"label":"Large","base":26}] as const).map(({ label, base }) => <div key={label}><span>{label}</span><strong>{movementValue(base, settings.pointerScalePercent)}</strong></div>)}
          </div>
        </div>
      </Disclosure></fieldset>
      <div className="repeat-settings">
        <Toggle label="Repeat mouse movement" checked={settings.mouseRepeatEnabled} onChange={(value) => update("mouseRepeatEnabled", value)} />
        <div className="repeat-options">
          <OptionGroup<number> legend="Movement interval" columns="four" disabled={!settings.mouseRepeatEnabled} options={secondsOptions(repeatIntervalOptions)} value={settings.moveRepeatIntervalMs} onChange={(next) => update("moveRepeatIntervalMs", next)} />
          <OptionGroup<number> legend="Movement acceleration" columns="four" disabled={!settings.mouseRepeatEnabled} options={accelerationOptions} value={settings.mouseRepeatAccelerationDurationMs} onChange={(next) => update("mouseRepeatAccelerationDurationMs", next)} />
          <OptionGroup<number> legend="Scroll interval" columns="four" disabled={!settings.mouseRepeatEnabled} options={secondsOptions(repeatIntervalOptions)} value={settings.scrollRepeatIntervalMs} onChange={(next) => update("scrollRepeatIntervalMs", next)} />
        </div>
      </div>
      <div className="repeat-settings">
        <Toggle label="Repeat held keys" checked={settings.keyRepeatEnabled} onChange={(value) => update("keyRepeatEnabled", value)} />
        <div className="repeat-options">
          <OptionGroup<number> legend="Delay before repeating" columns="four" disabled={!settings.keyRepeatEnabled} options={keyRepeatDelayOptions} value={settings.keyRepeatInitialDelayMs} onChange={(next) => update("keyRepeatInitialDelayMs", next)} />
          <OptionGroup<number> legend="Key interval" columns="four" disabled={!settings.keyRepeatEnabled} options={secondsOptions(repeatIntervalOptions)} value={settings.keyRepeatIntervalMs} onChange={(next) => update("keyRepeatIntervalMs", next)}
            note={{ about: "key repeat", summary: "Held navigation keys repeat, like on a keyboard.", detail: "Applies to the arrow keys, Tab, Backspace, Delete, Page Up, and Page Down." }} />
        </div>
      </div>
      <div className="repeat-settings dwell-settings">
        <Toggle label="Dwell to click" checked={settings.dwellClickEnabled} onChange={(value) => update("dwellClickEnabled", value)} />
        <div className="repeat-options">
          <OptionGroup<number> legend="Dwell delay" columns="five" disabled={!settings.dwellClickEnabled} options={secondsOptions(dwellDelayOptions)} value={settings.dwellClickDelayMs} onChange={(next) => update("dwellClickDelayMs", next)}
            note={{ summary: "After Android pointer movement stops, a countdown appears and performs one left click. Move again to rearm it." }} />
        </div>
      </div>
  </SettingGroup>;
}
