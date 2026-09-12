import {
  type ScanningController,
  type PointScanConfig,
  validSwitches,
} from "../scanning/useScanning";

import { SettingGroup, Toggle, OptionGroup, secondsOptions } from "./controls";

const keys = [
  "Space",
  "Enter",
  "Backspace",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  ...Array.from({ length: 24 }, (_, i) => `F${i + 1}`),
];

const phases = {
  idle: "Ready to begin",
  row: "Choose a row",
  cell: "Choose a cell",
  x: "Choose the horizontal position",
  y: "Choose the vertical position",
};

export function ScanningSection({
  controller,
}: {
  controller: ScanningController;
}) {
  const {
    state,
    config,
    pending,
    error,
    toggling,
    update,
    retry,
    toggle,
    unsaved,
  } = controller;

  const disabled = !state?.supported || !!state.enabled || toggling;

  return (
    <>
      <SettingGroup
        title="Scanning"
        description="Use switches connected to this computer to select a point on the screen."
      >
        <p>
          Focus the application you want to use, then press Select to scan the
          display under the pointer. Choose the X position, then the Y position
          to click once.
        </p>

        <p>
          Switch keys are reserved while enabled. Escape stops scanning and
          releases them. Disconnect Android before enabling local point scan.
        </p>

        <p role="status">
          {state?.enabled
            ? `${state.paused ? "Paused. " : ""}${phases[state.phase]}.`
            : (state?.message ?? "Loading point scan...")}
        </p>

        <button
          type="button"
          disabled={
            toggling ||
            !state?.supported ||
            (!state.enabled && !validSwitches(config))
          }
          onClick={toggle}
        >
          {state?.enabled ? "Disable point scan" : "Enable point scan"}
        </button>

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
        title="Switch controls"
        description="Movement and switch actions shared by scanning techniques."
      >
        <Toggle
          label="Automatic scanning"
          checked={config.automatic}
          disabled={disabled}
          onChange={(value) => update("automatic", value)}
        />

        {(
          [
            ["selectKey", "Select switch"],
            ["nextKey", "Forward switch"],
            ["backKey", "Backward switch"],
            ["pauseKey", "Pause / resume switch"],
          ] as const
        ).map(([key, label]) => (
          <label className="exact-speed" key={key}>
            <span>{label}</span>
            <select
              disabled={disabled}
              value={config[key]}
              onChange={(event) => update(key, event.target.value)}
            >
              {keys.map((value) => (
                <option key={value}>{value}</option>
              ))}
            </select>
          </label>
        ))}

        <p>
          Select takes effect on release. Holding Select freezes scanning.
          Forward and Backward step once and set direction. After clicking,
          press Select to start again.
        </p>
      </SettingGroup>

      <SettingGroup
        title="Point scan"
        description="Choose how the scanning lines find a point. Disable scanning to change these settings."
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

        <p>Settings are saved, but scanning stays off when the app restarts.</p>
      </SettingGroup>
    </>
  );
}
