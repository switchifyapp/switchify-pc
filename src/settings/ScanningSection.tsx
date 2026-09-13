import {
  type ScanningController,
  type PointScanConfig,
  validSwitches,
} from "../scanning/useScanning";

import { SettingGroup, Toggle, OptionGroup, secondsOptions } from "./controls";

const phases = {
  idle: "Ready to begin",
  menu: "Choose an action at the selected point",
  menuSuspended: "Select to resume the action menu",
  dragDestination: "Choose drag destination",
  dragConfirmation: "Confirm drag",
  executing: "Performing drag",
  row: "Choose a row",
  cell: "Choose a cell",
  rowEscape: "Select to return to rows",
  x: "Choose the horizontal position",
  y: "Choose the vertical position",
};

export function ScanningSection({
  controller,
}: {
  controller: ScanningController;
}) {
  const { state, config, pending, error, update, retry, unsaved } = controller;

  const disabled = !state?.supported;

  return (
    <>
      <SettingGroup
        title="Scanning"
        description="Use switches connected to this computer to select a point on the screen."
      >
        <p>
          Focus the application you want to use, then press Select to scan the
          display under the pointer. Choose the X position, then the Y position
          to open the action menu. Choose a click, scroll, or drag action using your switches.
        </p>

        <p>
          Scanning is on whenever a switch has the Select action, and its keys
          stay reserved while Switchify runs. Escape resets the scan. Android
          connections pause local scanning until they end.
        </p>
        <p className="setting-note">
          On Windows, use switch keys without Shift, Ctrl, Alt or Windows held.
          Key releases can still reach other applications.
        </p>

        <p role="status">
          {state?.enabled
            ? `${state.paused ? "Paused. " : ""}${phases[state.phase]}.`
            : (state?.message ?? "Loading point scan...")}
        </p>

        <p role="status">
          {pending
            ? "Saving scanning settings..."
            : unsaved
              ? "Scanning settings have unsaved changes."
              : "Scanning settings save automatically."}
        </p>

        {error && <p role="alert">{error}</p>}

        {error && unsaved && (
          <button
            type="button"
            className="secondary"
            disabled={!!pending || !validSwitches(config)}
            onClick={retry}
          >
            Retry save
          </button>
        )}

        {!validSwitches(config) && (
          <p role="alert">
            Each switch action needs a different key. Escape is reserved for
            cancel.
          </p>
        )}
      </SettingGroup>

      <SettingGroup
        title="Scan movement"
        description="Movement and switch actions shared by scanning techniques."
      >
        <Toggle
          label="Automatic scanning"
          checked={config.automatic}
          disabled={disabled}
          onChange={(value) => update("automatic", value)}
        />

        <p>Assign switch actions in the Switches tab. All actions run on release. Holding a switch freezes movement. After clicking, or after three passes without a selection, use Select to start again.</p>

      </SettingGroup>

      <SettingGroup
        title="Point scan"
        description="Choose how the scanning lines find a point. Changes apply straight away."
      >
        <OptionGroup<PointScanConfig["mode"]>
          legend="Mode"
          disabled={disabled}
          value={config.mode}
          onChange={(value) => update("mode", value)}
          options={[
            { value: "line", label: "Line only" },
            { value: "grid", label: "Grid then line" },
          ]}
        />

        <OptionGroup<number>
          legend="Line speed"
          columns="five"
          disabled={disabled}
          value={config.speed}
          onChange={(value) => update("speed", value)}
          options={["Very slow", "Slow", "Medium", "Fast", "Very fast"].map(
            (label, value) => ({ label, value }),
          )}
        />

        {config.mode === "grid" && (
          <>
            <label className="exact-speed">
              <span>Grid size</span>
              <select
                disabled={disabled}
                value={config.gridSize}
                onChange={(event) =>
                  update("gridSize", Number(event.target.value))
                }
              >
                {Array.from({ length: 9 }, (_, i) => i + 2).map((n) => (
                  <option key={n} value={n}>
                    {n} × {n}
                  </option>
                ))}
              </select>
            </label>

            <OptionGroup<number>
              legend="Grid interval"
              disabled={disabled}
              value={config.blockIntervalMs}
              onChange={(value) => update("blockIntervalMs", value)}
              options={secondsOptions([
                250, 500, 750, 1000, 1500, 2000, 3000, 4000, 5000,
              ])}
            />
          </>
        )}

        <p>Settings are saved and scanning resumes when the app restarts.</p>
      </SettingGroup>
    </>
  );
}
