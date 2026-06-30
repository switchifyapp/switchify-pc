export type PointerMovementSizeKey = 'small' | 'medium' | 'large';

export type PointerMovementSettings = {
  scalePercent: number;
};

export const BASE_POINTER_MOVEMENT_PERCENTAGES: Record<PointerMovementSizeKey, number> = {
  small: 4.5,
  medium: 12,
  large: 26
};

export const DEFAULT_POINTER_MOVEMENT_SETTINGS: PointerMovementSettings = {
  scalePercent: 100
};

export const POINTER_MOVEMENT_SCALE_MIN = 25;
export const POINTER_MOVEMENT_SCALE_MAX = 200;
export const POINTER_MOVEMENT_SCALE_STEP = 5;

const pointerMovementSizeKeys: PointerMovementSizeKey[] = ['small', 'medium', 'large'];
const DISPLAY_PERCENTAGE_MIN = 1;
const DISPLAY_PERCENTAGE_MAX = 50;
const DISPLAY_PERCENTAGE_STEP = 0.5;

export function normalizePointerMovementSettings(value: unknown): PointerMovementSettings {
  if (!value || typeof value !== 'object') {
    return { ...DEFAULT_POINTER_MOVEMENT_SETTINGS };
  }

  const candidate = value as Partial<PointerMovementSettings> & {
    percentages?: unknown;
    multipliers?: unknown;
  };

  if ('scalePercent' in candidate) {
    return { scalePercent: normalizeScale(candidate.scalePercent) };
  }

  if (candidate.percentages && typeof candidate.percentages === 'object') {
    return { scalePercent: normalizeScale(scaleFromPercentages(candidate.percentages)) };
  }

  if (candidate.multipliers && typeof candidate.multipliers === 'object') {
    return { scalePercent: normalizeScale(scaleFromLegacyMultipliers(candidate.multipliers)) };
  }

  return { ...DEFAULT_POINTER_MOVEMENT_SETTINGS };
}

export function pointerMovementScalePercentFor(settings: PointerMovementSettings): number {
  return normalizePointerMovementSettings(settings).scalePercent;
}

export function pointerMovementPercentageFor(
  settings: PointerMovementSettings,
  size: PointerMovementSizeKey
): number {
  const scale = pointerMovementScalePercentFor(settings) / 100;
  return normalizeDisplayPercentage(BASE_POINTER_MOVEMENT_PERCENTAGES[size] * scale);
}

export function pointerMovementFractionFor(
  settings: PointerMovementSettings,
  size: PointerMovementSizeKey
): number {
  return pointerMovementPercentageFor(settings, size) / 100;
}

function scaleFromPercentages(value: unknown): number {
  const percentages = value as Partial<Record<PointerMovementSizeKey, unknown>>;
  const scales = pointerMovementSizeKeys
    .map((size) => {
      const percentage = percentages[size];
      if (typeof percentage !== 'number' || !Number.isFinite(percentage)) return null;
      return (normalizeDisplayPercentage(percentage) / BASE_POINTER_MOVEMENT_PERCENTAGES[size]) * 100;
    })
    .filter((scale): scale is number => scale !== null);

  if (scales.length === 0) return DEFAULT_POINTER_MOVEMENT_SETTINGS.scalePercent;
  return scales.reduce((sum, scale) => sum + scale, 0) / scales.length;
}

function scaleFromLegacyMultipliers(value: unknown): number {
  const multipliers = value as Partial<Record<PointerMovementSizeKey, unknown>>;
  const scales = pointerMovementSizeKeys
    .map((size) => {
      const multiplier = multipliers[size];
      return typeof multiplier === 'number' && Number.isFinite(multiplier) ? multiplier : null;
    })
    .filter((scale): scale is number => scale !== null);

  if (scales.length === 0) return DEFAULT_POINTER_MOVEMENT_SETTINGS.scalePercent;
  return scales.reduce((sum, scale) => sum + scale, 0) / scales.length;
}

function normalizeScale(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return DEFAULT_POINTER_MOVEMENT_SETTINGS.scalePercent;
  }

  return clamp(roundToStep(value, POINTER_MOVEMENT_SCALE_STEP), POINTER_MOVEMENT_SCALE_MIN, POINTER_MOVEMENT_SCALE_MAX);
}

function normalizeDisplayPercentage(value: number): number {
  return clamp(roundToStep(value, DISPLAY_PERCENTAGE_STEP), DISPLAY_PERCENTAGE_MIN, DISPLAY_PERCENTAGE_MAX);
}

function roundToStep(value: number, step: number): number {
  return Math.round(value / step) * step;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}
