export type PointerMovementSizeKey = 'small' | 'medium' | 'large';

export type PointerMovementSettings = {
  percentages: Record<PointerMovementSizeKey, number>;
};

export const DEFAULT_POINTER_MOVEMENT_SETTINGS: PointerMovementSettings = {
  percentages: {
    small: 4.5,
    medium: 12,
    large: 26
  }
};

export const POINTER_MOVEMENT_PERCENTAGE_MIN = 1;
export const POINTER_MOVEMENT_PERCENTAGE_MAX = 50;
export const POINTER_MOVEMENT_PERCENTAGE_STEP = 0.5;

const pointerMovementSizeKeys: PointerMovementSizeKey[] = ['small', 'medium', 'large'];

export function normalizePointerMovementSettings(value: unknown): PointerMovementSettings {
  if (!value || typeof value !== 'object') {
    return cloneDefaultSettings();
  }

  const candidate = value as Partial<PointerMovementSettings>;
  const percentages = candidate.percentages;
  if (percentages && typeof percentages === 'object') {
    return {
      percentages: {
        small: normalizePercentage((percentages as Partial<Record<PointerMovementSizeKey, unknown>>).small, 'small'),
        medium: normalizePercentage((percentages as Partial<Record<PointerMovementSizeKey, unknown>>).medium, 'medium'),
        large: normalizePercentage((percentages as Partial<Record<PointerMovementSizeKey, unknown>>).large, 'large')
      }
    };
  }

  const multipliers = (candidate as { multipliers?: unknown }).multipliers;
  if (!multipliers || typeof multipliers !== 'object') {
    return cloneDefaultSettings();
  }

  return {
    percentages: {
      small: normalizeLegacyMultiplier(
        (multipliers as Partial<Record<PointerMovementSizeKey, unknown>>).small,
        'small'
      ),
      medium: normalizeLegacyMultiplier(
        (multipliers as Partial<Record<PointerMovementSizeKey, unknown>>).medium,
        'medium'
      ),
      large: normalizeLegacyMultiplier(
        (multipliers as Partial<Record<PointerMovementSizeKey, unknown>>).large,
        'large'
      )
    }
  };
}

export function pointerMovementPercentageFor(
  settings: PointerMovementSettings,
  size: PointerMovementSizeKey
): number {
  return normalizePointerMovementSettings(settings).percentages[size];
}

export function pointerMovementFractionFor(
  settings: PointerMovementSettings,
  size: PointerMovementSizeKey
): number {
  return pointerMovementPercentageFor(settings, size) / 100;
}

function normalizePercentage(value: unknown, size: PointerMovementSizeKey): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return DEFAULT_POINTER_MOVEMENT_SETTINGS.percentages[size];
  }

  return clamp(roundToStep(value), POINTER_MOVEMENT_PERCENTAGE_MIN, POINTER_MOVEMENT_PERCENTAGE_MAX);
}

function normalizeLegacyMultiplier(value: unknown, size: PointerMovementSizeKey): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    return DEFAULT_POINTER_MOVEMENT_SETTINGS.percentages[size];
  }

  return normalizePercentage((DEFAULT_POINTER_MOVEMENT_SETTINGS.percentages[size] * value) / 100, size);
}

function roundToStep(value: number): number {
  return Math.round(value / POINTER_MOVEMENT_PERCENTAGE_STEP) * POINTER_MOVEMENT_PERCENTAGE_STEP;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function cloneDefaultSettings(): PointerMovementSettings {
  return {
    percentages: Object.fromEntries(
      pointerMovementSizeKeys.map((size) => [size, DEFAULT_POINTER_MOVEMENT_SETTINGS.percentages[size]])
    ) as Record<PointerMovementSizeKey, number>
  };
}
