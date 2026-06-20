import { describe, expect, it } from 'vitest';
import {
  calculateNativeScrollDelta,
  calculateScaledMouseTarget,
  toLibnutKeyboardKey,
  toLibnutMouseToggle
} from './libnut-win32-adapter';
import {
  createWindowControlScript,
  toWindowsWindowControlStrategy
} from './windows-window-control';

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

describe('toLibnutMouseToggle', () => {
  it('maps held-button state to libnut toggle values', () => {
    expect(toLibnutMouseToggle(true)).toBe('down');
    expect(toLibnutMouseToggle(false)).toBe('up');
  });
});

describe('toLibnutKeyboardKey', () => {
  it('maps navigation keys to libnut key values', () => {
    expect(toLibnutKeyboardKey('PageUp')).toBe('pageup');
  });

  it('maps function keys to libnut key values', () => {
    expect(toLibnutKeyboardKey('F1')).toBe('f1');
    expect(toLibnutKeyboardKey('F12')).toBe('f12');
  });
});

describe('toWindowsWindowControlStrategy', () => {
  it('leaves app switching on the known-good libnut path', () => {
    expect(toWindowsWindowControlStrategy('switchNext')).toBeNull();
    expect(toWindowsWindowControlStrategy('switchPrevious')).toBeNull();
  });

  it('maps task view to the shell task view strategy', () => {
    expect(toWindowsWindowControlStrategy('taskView')).toEqual({ kind: 'taskView' });
  });

  it('maps show desktop to the shell desktop strategy', () => {
    expect(toWindowsWindowControlStrategy('showDesktop')).toEqual({ kind: 'showDesktop' });
  });

  it('maps close focused to a foreground window operation', () => {
    expect(toWindowsWindowControlStrategy('closeFocused')).toEqual({ kind: 'foregroundWindow', operation: 'close' });
  });

  it('maps minimize focused to a foreground window operation', () => {
    expect(toWindowsWindowControlStrategy('minimizeFocused')).toEqual({
      kind: 'foregroundWindow',
      operation: 'minimize'
    });
  });

  it('maps maximize focused to a foreground window operation', () => {
    expect(toWindowsWindowControlStrategy('maximizeFocused')).toEqual({
      kind: 'foregroundWindow',
      operation: 'maximizeOrRestore'
    });
  });
});

describe('createWindowControlScript', () => {
  it('uses shell integration for task view', () => {
    expect(createWindowControlScript({ kind: 'taskView' })).toBe(
      "Start-Process explorer.exe 'shell:::{3080F90E-D7AD-11D9-BD98-0000947B0257}'"
    );
  });

  it('uses shell integration for show desktop', () => {
    expect(createWindowControlScript({ kind: 'showDesktop' })).toBe(
      '(New-Object -ComObject Shell.Application).MinimizeAll()'
    );
  });

  it('uses direct foreground window APIs for close, minimize, and maximize', () => {
    const closeScript = createWindowControlScript({ kind: 'foregroundWindow', operation: 'close' });
    const minimizeScript = createWindowControlScript({ kind: 'foregroundWindow', operation: 'minimize' });
    const maximizeScript = createWindowControlScript({ kind: 'foregroundWindow', operation: 'maximizeOrRestore' });

    expect(closeScript).toContain('PostMessage($handle, 0x0010');
    expect(minimizeScript).toContain('ShowWindow($handle, 6)');
    expect(maximizeScript).toContain('IsZoomed($handle)');
    expect(maximizeScript).toContain('ShowWindow($handle, 9)');
    expect(maximizeScript).toContain('ShowWindow($handle, 3)');
  });
});
