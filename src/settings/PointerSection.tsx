import { Button, Input, MoreOptions } from "../ui/controls";
import type { AppSettings } from "../types";
import {
  OptionGroup, SettingGroup, Toggle, accelerationOptions, repeatIntervalOptions,
  secondsOptions, type SettingsUpdate,
} from "./controls";
import { speedLevelLabel, speedPercent } from "./pointerSpeedScale";

export function PointerSection({ settings, update }: { settings: AppSettings; update: SettingsUpdate }) {
  const percent = settings.pointerScalePercent;
  const level = speedLevelLabel(percent);
  return <SettingGroup title="Movement" description="">
      <fieldset className="pointer-speed"><legend>Pointer speed <strong>Level {level} of 10</strong></legend>
        <p>Move the slider toward Fast to cover more distance with each step in Mouse scanning and Remote.</p>
        <Input type="range" className="pointer-speed-slider" aria-label="Pointer speed" aria-valuetext={`Level ${level} of 10`} min={1} max={10} step={0.1} value={level} onChange={(event) => {
          const next = speedPercent(Number(event.target.value));
          if (next !== percent) update("pointerScalePercent", next);
        }} />
        <div className="pointer-speed-ends" aria-hidden="true"><span>Slow</span><span>Fast</span></div>
        <MoreOptions label="Fine tune speed"><div className="pointer-speed-fine-tune">
          <p>Adjust the speed in small steps.</p>
          <div className="pointer-speed-steps">
            <Button className="secondary" disabled={percent <= 5} onClick={() => update("pointerScalePercent", percent - 5)}>Slower</Button>
            <Button className="secondary" disabled={percent >= 1350} onClick={() => update("pointerScalePercent", percent + 5)}>Faster</Button>
          </div>
        </div></MoreOptions>
      </fieldset>
      <div className="repeat-settings">
        <Toggle label="Repeat mouse movement" checked={settings.mouseRepeatEnabled} onChange={(value) => update("mouseRepeatEnabled", value)} />
        <MoreOptions label="Repeat timing"><div className="repeat-options">
          <OptionGroup<number> legend="Movement interval" columns="four" disabled={!settings.mouseRepeatEnabled} options={secondsOptions(repeatIntervalOptions)} value={settings.moveRepeatIntervalMs} onChange={(next) => update("moveRepeatIntervalMs", next)} />
          <OptionGroup<number> legend="Movement acceleration" columns="four" disabled={!settings.mouseRepeatEnabled} options={accelerationOptions} value={settings.mouseRepeatAccelerationDurationMs} onChange={(next) => update("mouseRepeatAccelerationDurationMs", next)} />
          <OptionGroup<number> legend="Scroll interval" columns="four" disabled={!settings.mouseRepeatEnabled} options={secondsOptions(repeatIntervalOptions)} value={settings.scrollRepeatIntervalMs} onChange={(next) => update("scrollRepeatIntervalMs", next)} />
        </div></MoreOptions>
      </div>
  </SettingGroup>;
}
