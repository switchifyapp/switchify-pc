import { describe, expect, it } from 'vitest';
import { calculateScaledMouseTarget } from './libnut-win32-adapter';

describe('calculateScaledMouseTarget', () => {
  it('applies the display scale factor to relative movement', () => {
    expect(calculateScaledMouseTarget({ x: 100, y: 200 }, { dx: 30, dy: -12 }, 2.5)).toEqual({
      x: 175,
      y: 170
    });
  });

  it('handles displays with negative coordinates', () => {
    expect(calculateScaledMouseTarget({ x: -300, y: 100 }, { dx: 80, dy: 40 }, 1.5)).toEqual({
      x: -180,
      y: 160
    });
  });

  it('falls back to unscaled movement for invalid scale values', () => {
    expect(calculateScaledMouseTarget({ x: 10, y: 20 }, { dx: 5, dy: 6 }, 0)).toEqual({
      x: 15,
      y: 26
    });
  });
});
