// The ranges timing settings accept, set with a − / + stepper. Each takes any
// multiple of its step between its minimum and maximum, in milliseconds.
// Mirrors src-tauri/src/timing.rs.
export type TimeRange = { min: number; max: number; step: number };

export const dwellClickDelayRange: TimeRange = { min: 100, max: 10000, step: 100 };
export const keyRepeatInitialDelayRange: TimeRange = { min: 0, max: 2000, step: 50 };
export const keyRepeatIntervalRange: TimeRange = { min: 100, max: 1000, step: 50 };
export const mouseRepeatIntervalRange: TimeRange = { min: 100, max: 2000, step: 50 };
export const mouseRepeatAccelerationRange: TimeRange = { min: 0, max: 3000, step: 100 };
// A set time between block hops; 0 keeps the mouse panel's scan speed.
export const mouseBlockIntervalRange: TimeRange = { min: 100, max: 10000, step: 100 };
export const switchAcceptRange: TimeRange = { min: 0, max: 2000, step: 50 };
export const switchIgnoreRepeatRange: TimeRange = { min: 0, max: 3000, step: 50 };
// The backend takes any millisecond in this range, as earlier builds did;
// the stepper moves on the step.
export const holdIntervalRange: TimeRange = { min: 250, max: 5000, step: 50 };
export const firstItemRange: TimeRange = { min: 0, max: 3000, step: 100 };

/** A time in seconds, without floating-point noise: 350 ms is "0.35 s". */
export function formatSeconds(ms: number) {
  return `${Number((ms / 1000).toFixed(3))} s`;
}

/** One step from `ms` in `direction`, kept on the step and inside the range. */
export function stepTime(ms: number, direction: -1 | 1, range: TimeRange) {
  const onStep = direction > 0 ? Math.floor(ms / range.step) * range.step : Math.ceil(ms / range.step) * range.step;
  return Math.min(range.max, Math.max(range.min, onStep + direction * range.step));
}
