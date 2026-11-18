import { Button, Input, MoreOptions } from "../ui/controls";
import { useEffect, useState, type ReactNode } from "react";
import type { AppSettings } from "../types";
import {
  OptionGroup, SettingGroup, Toggle, accelerationOptions, repeatIntervalOptions,
  secondsOptions, type SettingsUpdate,
} from "./controls";
import { speedLevel, speedLevelLabel, speedPercent } from "./pointerSpeedScale";

const blockSizeOptions: { value: AppSettings["mouseBlockSize"]; label: string }[] = [
  { value: "small", label: "Small" },
  { value: "medium", label: "Medium" },
  { value: "large", label: "Large" },
];

// 0 keeps the mouse panel's scan speed. Mirrors `BLOCK_INTERVALS_MS` in src-tauri/src/mouse_repeat.rs.
const blockSpeedOptions = [{ value: 0, label: "Scan speed" }, ...secondsOptions([500, 1000, 1500, 2000, 3000])];

export function PointerSection({ settings, update, children, title = "Movement", description = "", section }: { settings: AppSettings; update: SettingsUpdate; children?: ReactNode; title?: string; description?: string; section?: string }) {
  const percent = settings.pointerScalePercent;
  const level = speedLevelLabel(percent);
  const [sliderLevel, setSliderLevel] = useState(level);
  useEffect(() => { setSliderLevel(level); }, [level, percent]);
  const changeSpeed = (next: number) => {
    if (next !== percent) update("pointerScalePercent", next);
  };
  return <SettingGroup title={title} description={description} section={section}>
      <fieldset className="pointer-speed" data-setting="pointer-speed"><legend>Pointer speed <strong>Level {level} of 10</strong></legend>
        <p>Move the slider toward Fast to cover more distance with each step in Mouse scanning and Remote.</p>
        <Input type="range" className="pointer-speed-slider" aria-label="Pointer speed" aria-valuetext={`Level ${speedLevel(percent).toFixed(3)} of 10`} min={1} max={10} step={0.1} value={sliderLevel} onChange={(event) => {
          setSliderLevel(event.target.value);
          changeSpeed(speedPercent(Number(event.target.value)));
        }} onKeyDown={(event) => {
          const delta = event.key === "ArrowRight" || event.key === "ArrowUp" ? 5 : event.key === "ArrowLeft" || event.key === "ArrowDown" ? -5 : 0;
          if (!delta && event.key !== "Home" && event.key !== "End") return;
          event.preventDefault();
          const next = event.key === "Home" ? 5 : event.key === "End" ? 1350 : Math.min(1350, Math.max(5, percent + delta));
          setSliderLevel(speedLevelLabel(next));
          changeSpeed(next);
        }} />
        <div className="pointer-speed-ends" aria-hidden="true"><span>Slow</span><span>Fast</span></div>
        <MoreOptions label="Fine tune speed"><div className="pointer-speed-fine-tune">
          <p>Adjust the speed in small steps.</p>
          <p role="status" aria-live="polite">Fine-tuned level {speedLevel(percent).toFixed(3)} of 10</p>
          <div className="pointer-speed-steps">
            <Button className="secondary" disabled={percent <= 5} onClick={() => update("pointerScalePercent", percent - 5)}>Slower</Button>
            <Button className="secondary" disabled={percent >= 1350} onClick={() => update("pointerScalePercent", percent + 5)}>Faster</Button>
          </div>
        </div></MoreOptions>
      </fieldset>
      <div className="block-movement" data-setting="move-in-blocks">
        <Toggle label="Move in blocks" checked={settings.mouseMovement === "blocks"} onChange={(value) => update("mouseMovement", value ? "blocks" : "smooth")} />
        <p className="setting-note">The mouse panel's arrows jump the pointer a block at a time instead of gliding. You can also switch this with the Blocks tile on the panel's Speed and more page, or with a switch set to Move in blocks on / off.</p>
        <OptionGroup<AppSettings["mouseBlockSize"]> legend="Block size" columns="three" disabled={false} options={blockSizeOptions} value={settings.mouseBlockSize} onChange={(next) => update("mouseBlockSize", next)} />
        <OptionGroup<number> legend="Block speed" columns="three" disabled={!settings.mouseRepeatEnabled} options={blockSpeedOptions} value={settings.mouseBlockIntervalMs} onChange={(next) => update("mouseBlockIntervalMs", next)}
          note={{ about: "block speed", summary: settings.mouseRepeatEnabled ? "How long the pointer waits between blocks when it keeps moving." : "Turn on Repeat mouse movement and scrolling to use this.", detail: "Scan speed waits as long as the mouse panel takes to move between tiles, so you have as long to stop the pointer as to choose a tile. Pick a time to make blocks move faster or slower than that." }} />
      </div>
      <div className="repeat-settings" data-setting="repeat">
        <Toggle label="Repeat mouse movement and scrolling" checked={settings.mouseRepeatEnabled} onChange={(value) => update("mouseRepeatEnabled", value)} />
        {children}
        <MoreOptions label="Repeat timing"><div className="repeat-options">
          <OptionGroup<number> legend="Movement interval" columns="four" disabled={!settings.mouseRepeatEnabled} options={secondsOptions(repeatIntervalOptions)} value={settings.moveRepeatIntervalMs} onChange={(next) => update("moveRepeatIntervalMs", next)} />
          <OptionGroup<number> legend="Movement acceleration" columns="four" disabled={!settings.mouseRepeatEnabled} options={accelerationOptions} value={settings.mouseRepeatAccelerationDurationMs} onChange={(next) => update("mouseRepeatAccelerationDurationMs", next)} />
          <OptionGroup<number> legend="Scroll interval" columns="four" disabled={!settings.mouseRepeatEnabled} options={secondsOptions(repeatIntervalOptions)} value={settings.scrollRepeatIntervalMs} onChange={(next) => update("scrollRepeatIntervalMs", next)} />
        </div></MoreOptions>
      </div>
  </SettingGroup>;
}
