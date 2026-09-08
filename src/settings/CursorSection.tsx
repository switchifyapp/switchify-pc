import type { AppSettings } from "../types";
import {
  OptionGroup, SettingGroup, Toggle, overlaySizeOptions, overlayVisibilityOptions,
  type SettingsUpdate,
} from "./controls";

export function CursorSection({ settings, update }: { settings: AppSettings; update: SettingsUpdate }) {
  return <SettingGroup title="Cursor appearance" description="On-screen cursor overlay shown while an Android device is controlling this computer.">
        <Toggle label="Show cursor overlay" checked={settings.cursorOverlayEnabled} onChange={(value) => update("cursorOverlayEnabled", value)} />
        <div className="overlay-options">
          <OptionGroup<AppSettings["cursorOverlayVisibility"]> legend="Overlay visibility" disabled={!settings.cursorOverlayEnabled} options={overlayVisibilityOptions} value={settings.cursorOverlayVisibility} onChange={(next) => update("cursorOverlayVisibility", next)}
            note={{ about: "overlay visibility", summary: "Choose when the overlay stays on screen.", detail: "On input hides shortly after pointer activity stops. While controlling stays visible until the session ends." }} />
          <OptionGroup<AppSettings["cursorOverlaySize"]> legend="Overlay size" columns="three" disabled={!settings.cursorOverlayEnabled} options={overlaySizeOptions} value={settings.cursorOverlaySize} onChange={(next) => update("cursorOverlaySize", next)} />
          <fieldset disabled={!settings.cursorOverlayEnabled}><legend>Overlay color</legend><div className="color-options">
            {(["red", "green", "blue", "yellow", "white"] as const).map((value) => <label key={value} title={value[0].toUpperCase() + value.slice(1)}><input type="radio" name="overlay-color" value={value} checked={settings.cursorOverlayColor === value} onChange={() => update("cursorOverlayColor", value)} /><span className={`color-swatch ${value}`} /><span className="sr-only">{value[0].toUpperCase() + value.slice(1)}</span></label>)}
          </div></fieldset>
          <Toggle label="Show crosshairs" disabled={!settings.cursorOverlayEnabled} checked={settings.cursorCrosshairs} onChange={(value) => update("cursorCrosshairs", value)} />
        </div>
  </SettingGroup>;
}
