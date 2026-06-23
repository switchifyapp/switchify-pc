import { describe, expect, it } from 'vitest';
import {
  DEFAULT_POINTER_MOVEMENT_SETTINGS,
  normalizePointerMovementSettings,
  pointerMovementFractionFor,
  pointerMovementPercentageFor
} from './pointer-movement-settings';

describe('normalizePointerMovementSettings', () => {
  it('uses defaults for missing input', () => {
    expect(normalizePointerMovementSettings(null)).toEqual(DEFAULT_POINTER_MOVEMENT_SETTINGS);
  });

  it('preserves valid percentages', () => {
    expect(
      normalizePointerMovementSettings({
        percentages: {
          small: 3,
          medium: 12.5,
          large: 30
        }
      })
    ).toEqual({
      percentages: {
        small: 3,
        medium: 12.5,
        large: 30
      }
    });
  });

  it('fills missing sizes with defaults', () => {
    expect(
      normalizePointerMovementSettings({
        percentages: {
          small: 6
        }
      })
    ).toEqual({
      percentages: {
        small: 6,
        medium: 12,
        large: 26
      }
    });
  });

  it('clamps values below the minimum', () => {
    expect(
      normalizePointerMovementSettings({
        percentages: {
          small: 0.2
        }
      }).percentages.small
    ).toBe(1);
  });

  it('clamps values above the maximum', () => {
    expect(
      normalizePointerMovementSettings({
        percentages: {
          large: 100
        }
      }).percentages.large
    ).toBe(50);
  });

  it('enforces ordered percentages with a minimum half percent gap', () => {
    expect(
      normalizePointerMovementSettings({
        percentages: {
          small: 12,
          medium: 12,
          large: 12
        }
      })
    ).toEqual({
      percentages: {
        small: 11.5,
        medium: 12,
        large: 12.5
      }
    });
  });

  it('clamps small below medium', () => {
    expect(
      normalizePointerMovementSettings({
        percentages: {
          small: 20,
          medium: 12,
          large: 26
        }
      })
    ).toEqual({
      percentages: {
        small: 11.5,
        medium: 12,
        large: 26
      }
    });
  });

  it('clamps medium above small', () => {
    expect(
      normalizePointerMovementSettings({
        percentages: {
          small: 4.5,
          medium: 3,
          large: 26
        }
      })
    ).toEqual({
      percentages: {
        small: 4.5,
        medium: 5,
        large: 26
      }
    });
  });

  it('clamps medium below large', () => {
    expect(
      normalizePointerMovementSettings({
        percentages: {
          small: 4.5,
          medium: 40,
          large: 26
        }
      })
    ).toEqual({
      percentages: {
        small: 4.5,
        medium: 25.5,
        large: 26
      }
    });
  });

  it('clamps large above medium', () => {
    expect(
      normalizePointerMovementSettings({
        percentages: {
          small: 4.5,
          medium: 12,
          large: 10
        }
      })
    ).toEqual({
      percentages: {
        small: 4.5,
        medium: 12,
        large: 12.5
      }
    });
  });

  it('shifts values down from the top when order would exceed the maximum', () => {
    expect(
      normalizePointerMovementSettings({
        percentages: {
          small: 49.8,
          medium: 50,
          large: 50
        }
      })
    ).toEqual({
      percentages: {
        small: 49,
        medium: 49.5,
        large: 50
      }
    });
  });

  it('rounds values to the nearest half percent', () => {
    expect(
      normalizePointerMovementSettings({
        percentages: {
          medium: 12.3
        }
      }).percentages.medium
    ).toBe(12.5);
  });

  it('falls back for non-finite and non-number values', () => {
    expect(
      normalizePointerMovementSettings({
        percentages: {
          small: Number.NaN,
          medium: '12',
          large: Number.POSITIVE_INFINITY
        }
      })
    ).toEqual(DEFAULT_POINTER_MOVEMENT_SETTINGS);
  });

  it('migrates legacy multiplier settings to screen percentages', () => {
    expect(
      normalizePointerMovementSettings({
        multipliers: {
          small: 200,
          medium: 50,
          large: 100
        }
      })
    ).toEqual({
      percentages: {
        small: 9,
        medium: 9.5,
        large: 26
      }
    });
  });
});

describe('pointer movement setting helpers', () => {
  it('returns normalized percentages', () => {
    expect(pointerMovementPercentageFor({ percentages: { small: 0.2, medium: 12, large: 26 } }, 'small')).toBe(1);
  });

  it('returns fractions for normalized percentages', () => {
    expect(pointerMovementFractionFor(DEFAULT_POINTER_MOVEMENT_SETTINGS, 'small')).toBe(0.045);
    expect(pointerMovementFractionFor(DEFAULT_POINTER_MOVEMENT_SETTINGS, 'medium')).toBe(0.12);
    expect(pointerMovementFractionFor(DEFAULT_POINTER_MOVEMENT_SETTINGS, 'large')).toBe(0.26);
  });
});
