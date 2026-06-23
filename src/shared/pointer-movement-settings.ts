export type PointerMovementSizeKey = 'small' | 'medium' | 'large';

export type PointerMovementSettings = {
  multipliers: Record<PointerMovementSizeKey, number>;
};

export const DEFAULT_POINTER_MOVEMENT_SETTINGS: PointerMovementSettings = {
  multipliers: {
    small: 100,
    medium: 100,
    large: 100
  }
};

export const POINTER_MOVEMENT_MULTIPLIER_MIN = 50;
export const POINTER_MOVEMENT_MULTIPLIER_MAX = 200;
export const POINTER_MOVEMENT_MULTIPLIER_STEP = 5;

const pointerMovementSizeKeys: PointerMovementSizeKey[] = ['small', 'medium', 'large'];

export function normalizePointerMovementSettings(value: unknown): PointerMovementSettings {
  if (!value || typeof value !== 'object') {
    return cloneDefaultSettings();
  }

  const candidate = value as Partial<PointerMovementSettings>;
  const multipliers = candidate.multipliers;
  if (!multipliers || typeof multipliers !== 'object') {
    return cloneDefaultSettings();
  }

  return {
    multipliers: {
      small: normalizeMultiplier((multipliers as Partial<Record<PointerMovementSizeKey, unknown>>).small, 'small'),
      medium: normalizeMultiplier((multipliers as Partial<Record<PointerMovementSizeKey, unknown>>).medium, 'medium'),
      large: normalizeMultiplier((multipliers as Partial<Record<PointerMovementSizeKey, unknown>>).large, 'large')
    }
  };
}

export function pointerMovementMultiplierFor(
  settings: PointerMovementSettings,
  size: PointerMovementSizeKey
): number {
  return normalizePointerMovementSettings(settings).multipliers[size];
}

export function pointerMovementScaleFor(
  settings: PointerMovementSettings,
  size: PointerMovementSizeKey
): number {
  return pointerMovementMultiplierFor(settings, size) / 100;
}

function normalizeMultiplier(value: unknown, size: PointerMovementSizeKey): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return DEFAULT_POINTER_MOVEMENT_SETTINGS.multipliers[size];
  }

  return clamp(roundToStep(value), POINTER_MOVEMENT_MULTIPLIER_MIN, POINTER_MOVEMENT_MULTIPLIER_MAX);
}

function roundToStep(value: number): number {
  return Math.round(value / POINTER_MOVEMENT_MULTIPLIER_STEP) * POINTER_MOVEMENT_MULTIPLIER_STEP;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function cloneDefaultSettings(): PointerMovementSettings {
  return {
    multipliers: Object.fromEntries(
      pointerMovementSizeKeys.map((size) => [size, DEFAULT_POINTER_MOVEMENT_SETTINGS.multipliers[size]])
    ) as Record<PointerMovementSizeKey, number>
  };
}
