import { Button, Input, MoreOptions } from "../ui/controls";
import { useEffect, useId, useState } from "react";
import type { AppSettings } from "../types";
import {
  Disclosure, OptionGroup, SettingGroup, Toggle, accelerationOptions, dwellDelayOptions, keyRepeatDelayOptions,
  movementValue, pointerSpeedOptions, repeatIntervalOptions, secondsOptions,
  type SettingsUpdate,
} from "./controls";

export function PointerSection({ settings, update }: { settings: AppSettings; update: SettingsUpdate }) {
  // Keep a draft so the user can type a multi-digit speed without saving each
  // intermediate value or losing focus to a backend state update.
  const isPreset = (pointerSpeedOptions as readonly number[]).includes(settings.pointerScalePercent);
  const [showExact, setShowExact] = useState(!isPreset);
  const [speedDraft, setSpeedDraft] = useState(String(settings.pointerScalePercent));
  useEffect(() => { setSpeedDraft(String(settings.pointerScalePercent)); }, [settings.pointerScalePercent]);
  useEffect(() => { if (!isPreset) setShowExact(true); }, [isPreset]);
  const exactSpeedId = useId();
  const saveExactSpeed = () => {
    const entered = Number(speedDraft);
    if (!Number.isFinite(entered) || entered <= 0) {
      setSpeedDraft(String(settings.pointerScalePercent));
      return;
    }
    const next = Math.min(1350, Math.max(5, Math.round(entered / 5) * 5));
    setSpeedDraft(String(next));
    if (next !== settings.pointerScalePercent) update("pointerScalePercent", next);
  };
  return <SettingGroup title="Controls" description="">
      <fieldset className="pointer-speed"><legend>Pointer speed <strong>{settings.pointerScalePercent}%</strong></legend><p>100% is the original speed. Higher speeds help cross the screen faster in Mouse and Remote.</p><div className="segmented compact speed-presets">
        {pointerSpeedOptions.map((value) => <Button type="button" key={value} aria-label={`${value}% pointer speed`} aria-pressed={settings.pointerScalePercent === value} onClick={() => update("pointerScalePercent", value)}>{value}%</Button>)}
      </div><Disclosure label={showExact ? "Hide exact speed" : "Set an exact speed"} expanded={showExact} onToggle={() => setShowExact(!showExact)} controls={exactSpeedId}>
        <div id={exactSpeedId}>
          <label className="exact-speed"><span>Exact speed (%)</span><Input type="number" aria-label="Exact pointer speed" min={5} max={1350} step={5} value={speedDraft} onChange={(event) => setSpeedDraft(event.target.value)} onBlur={saveExactSpeed} onKeyDown={(event) => { if (event.key === "Enter") event.currentTarget.blur(); }} /></label>
          <div className="movement-values" aria-label="Pointer movement values">
            {([{"label":"Small","base":4.5},{"label":"Medium","base":12},{"label":"Large","base":26}] as const).map(({ label, base }) => <div key={label}><span>{label}</span><strong>{movementValue(base, settings.pointerScalePercent)}</strong></div>)}
          </div>
        </div>
      </Disclosure></fieldset>
      <div className="repeat-settings dwell-settings">
        <Toggle label="Click when I stop" checked={settings.dwellClickEnabled} onChange={(value) => update("dwellClickEnabled", value)} />
        <div className="repeat-options">
          <OptionGroup<number> legend="Wait before clicking" columns="five" disabled={!settings.dwellClickEnabled} options={secondsOptions(dwellDelayOptions)} value={settings.dwellClickDelayMs} onChange={(next) => update("dwellClickDelayMs", next)}
            note={{ summary: "After mobile pointer movement stops, a countdown appears and performs one left click. Move again to rearm it." }} />
        </div>
      </div>
      <MoreOptions>      <div className="repeat-settings">
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
</MoreOptions>
  </SettingGroup>;
}
