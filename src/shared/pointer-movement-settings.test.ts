import { describe, expect, it } from 'vitest';
import {
  DEFAULT_POINTER_MOVEMENT_SETTINGS,
  normalizePointerMovementSettings,
  pointerMovementFractionFor,
  pointerMovementPercentageFor,
  pointerMovementScalePercentFor
} from './pointer-movement-settings';

describe('normalizePointerMovementSettings', () => {
  it('uses defaults for missing input', () => {
    expect(normalizePointerMovementSettings(null)).toEqual(DEFAULT_POINTER_MOVEMENT_SETTINGS);
  });

  it('preserves valid scale percentages', () => {
    expect(normalizePointerMovementSettings({ scalePercent: 125 })).toEqual({ scalePercent: 125 });
  });

  it('clamps scale values below the minimum', () => {
    expect(normalizePointerMovementSettings({ scalePercent: 10 })).toEqual({ scalePercent: 25 });
  });

  it('clamps scale values above the maximum', () => {
    expect(normalizePointerMovementSettings({ scalePercent: 1000 })).toEqual({ scalePercent: 200 });
  });

  it('rounds scale values to the nearest five percent', () => {
    expect(normalizePointerMovementSettings({ scalePercent: 123 })).toEqual({ scalePercent: 125 });
  });

  it('falls back for non-finite and non-number values', () => {
    expect(normalizePointerMovementSettings({ scalePercent: Number.NaN })).toEqual(DEFAULT_POINTER_MOVEMENT_SETTINGS);
    expect(normalizePointerMovementSettings({ scalePercent: '125' })).toEqual(DEFAULT_POINTER_MOVEMENT_SETTINGS);
  });

  it('migrates percentage settings to one scale value', () => {
    expect(
      normalizePointerMovementSettings({
        percentages: {
          small: 9,
          medium: 24,
          large: 50
        }
      })
    ).toEqual({ scalePercent: 195 });
  });

  it('migrates legacy multiplier settings to one scale value', () => {
    expect(
      normalizePointerMovementSettings({
        multipliers: {
          small: 200,
          medium: 50,
          large: 100
        }
      })
    ).toEqual({ scalePercent: 115 });
  });
});

describe('pointer movement setting helpers', () => {
  it('returns normalized scale percentages', () => {
    expect(pointerMovementScalePercentFor({ scalePercent: 123 })).toBe(125);
  });

  it('returns derived movement percentages', () => {
    expect(pointerMovementPercentageFor({ scalePercent: 150 }, 'small')).toBe(7);
    expect(pointerMovementPercentageFor({ scalePercent: 150 }, 'medium')).toBe(18);
    expect(pointerMovementPercentageFor({ scalePercent: 150 }, 'large')).toBe(39);
  });

  it('returns fractions for derived movement percentages', () => {
    expect(pointerMovementFractionFor(DEFAULT_POINTER_MOVEMENT_SETTINGS, 'small')).toBe(0.045);
    expect(pointerMovementFractionFor(DEFAULT_POINTER_MOVEMENT_SETTINGS, 'medium')).toBe(0.12);
    expect(pointerMovementFractionFor(DEFAULT_POINTER_MOVEMENT_SETTINGS, 'large')).toBe(0.26);
  });
});
