import { describe, expect, it } from 'vitest';
import {
  DEFAULT_POINTER_MOVEMENT_SETTINGS,
  normalizePointerMovementSettings,
  pointerMovementMultiplierFor,
  pointerMovementScaleFor
} from './pointer-movement-settings';

describe('normalizePointerMovementSettings', () => {
  it('uses defaults for missing input', () => {
    expect(normalizePointerMovementSettings(null)).toEqual(DEFAULT_POINTER_MOVEMENT_SETTINGS);
  });

  it('preserves valid multipliers', () => {
    expect(
      normalizePointerMovementSettings({
        multipliers: {
          small: 75,
          medium: 125,
          large: 175
        }
      })
    ).toEqual({
      multipliers: {
        small: 75,
        medium: 125,
        large: 175
      }
    });
  });

  it('fills missing sizes with defaults', () => {
    expect(
      normalizePointerMovementSettings({
        multipliers: {
          small: 125
        }
      })
    ).toEqual({
      multipliers: {
        small: 125,
        medium: 100,
        large: 100
      }
    });
  });

  it('clamps values below the minimum', () => {
    expect(
      normalizePointerMovementSettings({
        multipliers: {
          small: 10
        }
      }).multipliers.small
    ).toBe(50);
  });

  it('clamps values above the maximum', () => {
    expect(
      normalizePointerMovementSettings({
        multipliers: {
          large: 1000
        }
      }).multipliers.large
    ).toBe(200);
  });

  it('rounds values to the nearest five percent', () => {
    expect(
      normalizePointerMovementSettings({
        multipliers: {
          medium: 123
        }
      }).multipliers.medium
    ).toBe(125);
  });

  it('falls back for non-finite and non-number values', () => {
    expect(
      normalizePointerMovementSettings({
        multipliers: {
          small: Number.NaN,
          medium: '150',
          large: Number.POSITIVE_INFINITY
        }
      })
    ).toEqual(DEFAULT_POINTER_MOVEMENT_SETTINGS);
  });
});

describe('pointer movement setting helpers', () => {
  it('returns normalized multipliers', () => {
    expect(pointerMovementMultiplierFor({ multipliers: { small: 49, medium: 101, large: 202 } }, 'small')).toBe(50);
  });

  it('returns scale factors for normalized multipliers', () => {
    expect(pointerMovementScaleFor({ multipliers: { small: 50, medium: 100, large: 200 } }, 'small')).toBe(0.5);
    expect(pointerMovementScaleFor({ multipliers: { small: 50, medium: 100, large: 200 } }, 'medium')).toBe(1);
    expect(pointerMovementScaleFor({ multipliers: { small: 50, medium: 100, large: 200 } }, 'large')).toBe(2);
  });
});
