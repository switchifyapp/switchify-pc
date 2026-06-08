import { describe, expect, it } from 'vitest';
import { calculateNativeScrollDelta, calculateScaledMouseTarget, toWindowControlShortcut } from './libnut-win32-adapter';

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

describe('calculateNativeScrollDelta', () => {
  it('scales vertical scroll by the native multiplier', () => {
    expect(calculateNativeScrollDelta({ dx: 0, dy: 5 })).toEqual({
      dx: 0,
      dy: 40
    });
  });

  it('scales horizontal scroll by the native multiplier', () => {
    expect(calculateNativeScrollDelta({ dx: 1, dy: 0 })).toEqual({
      dx: 8,
      dy: 0
    });
  });

  it('preserves negative scroll direction', () => {
    expect(calculateNativeScrollDelta({ dx: -2, dy: -5 })).toEqual({
      dx: -16,
      dy: -40
    });
  });

  it('preserves zero axes exactly', () => {
    expect(calculateNativeScrollDelta({ dx: 0, dy: 0 })).toEqual({
      dx: 0,
      dy: 0
    });
  });

  it('rounds fractional scaled values', () => {
    expect(calculateNativeScrollDelta({ dx: 1.4, dy: -1.6 })).toEqual({
      dx: 11,
      dy: -13
    });
  });

  it('keeps tiny nonzero scroll values at least one native unit', () => {
    expect(calculateNativeScrollDelta({ dx: 0.1, dy: -0.1 })).toEqual({
      dx: 1,
      dy: -1
    });
  });

  it('falls back to unscaled output for invalid multiplier values', () => {
    expect(calculateNativeScrollDelta({ dx: 2, dy: -3 }, 0)).toEqual({
      dx: 2,
      dy: -3
    });
    expect(calculateNativeScrollDelta({ dx: 2, dy: -3 }, Number.NaN)).toEqual({
      dx: 2,
      dy: -3
    });
  });
});

describe('toWindowControlShortcut', () => {
  it('maps switch next to Alt+Tab', () => {
    expect(toWindowControlShortcut('switchNext')).toEqual({ key: 'tab', modifiers: ['alt'] });
  });

  it('maps switch previous to Alt+Shift+Tab', () => {
    expect(toWindowControlShortcut('switchPrevious')).toEqual({ key: 'tab', modifiers: ['alt', 'shift'] });
  });

  it('maps task view to Win+Tab', () => {
    expect(toWindowControlShortcut('taskView')).toEqual({ key: 'tab', modifiers: ['command'] });
  });

  it('maps show desktop to Win+D', () => {
    expect(toWindowControlShortcut('showDesktop')).toEqual({ key: 'd', modifiers: ['command'] });
  });

  it('maps close focused to Alt+F4', () => {
    expect(toWindowControlShortcut('closeFocused')).toEqual({ key: 'f4', modifiers: ['alt'] });
  });

  it('maps minimize focused to Win+Down', () => {
    expect(toWindowControlShortcut('minimizeFocused')).toEqual({ key: 'down', modifiers: ['command'] });
  });

  it('maps maximize focused to Win+Up', () => {
    expect(toWindowControlShortcut('maximizeFocused')).toEqual({ key: 'up', modifiers: ['command'] });
  });
});
