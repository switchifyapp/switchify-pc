import { describe, expect, it } from 'vitest';
import {
  calculateNativeScrollDelta,
  calculateDisplayNormalizedMouseTarget,
  calculateScaledMouseTarget,
  inferPointerMovementSize,
  toLibnutKeyboardKey,
  toLibnutMouseToggle
} from './libnut-win32-adapter';
import {
  createWindowControlScript,
  toWindowsWindowControlStrategy
} from './windows-window-control';
import { createPointerMovementProfile } from './pointer-profile';
import type { PointerMovementSettings } from '../../shared/pointer-movement-settings';

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

describe('calculateDisplayNormalizedMouseTarget', () => {
  it('uses 1080p as the reference display size', () => {
    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 100, y: 200 },
        { dx: 128, dy: -64 },
        { bounds: { x: 0, y: 0, width: 1920, height: 1080 }, scaleFactor: 1 }
      )
    ).toEqual({
      x: 228,
      y: 136
    });
  });

  it('applies larger native movement on a 4K display at 1x scale', () => {
    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 100, y: 200 },
        { dx: 128, dy: 0 },
        { bounds: { x: 0, y: 0, width: 3840, height: 2160 }, scaleFactor: 1 }
      )
    ).toEqual({
      x: 356,
      y: 200
    });
  });

  it('combines high-DPI logical deltas with display resolution normalization', () => {
    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 100, y: 200 },
        { dx: 64, dy: 0 },
        { bounds: { x: 0, y: 0, width: 3840, height: 2160 }, scaleFactor: 2 }
      )
    ).toEqual({
      x: 356,
      y: 200
    });
  });

  it('uses the display short edge for ultrawide displays', () => {
    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 100, y: 200 },
        { dx: 128, dy: 0 },
        { bounds: { x: 0, y: 0, width: 3440, height: 1440 }, scaleFactor: 1 }
      )
    ).toEqual({
      x: 271,
      y: 200
    });
  });

  it('accounts for fractional scale factors', () => {
    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 100, y: 200 },
        { dx: 102, dy: 0 },
        { bounds: { x: 0, y: 0, width: 2560, height: 1440 }, scaleFactor: 1.25 }
      )
    ).toEqual({
      x: 270,
      y: 200
    });
  });

  it('handles displays with negative coordinates', () => {
    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: -300, y: 100 },
        { dx: 128, dy: 64 },
        { bounds: { x: -3840, y: 0, width: 3840, height: 2160 }, scaleFactor: 1 }
      )
    ).toEqual({
      x: -44,
      y: 228
    });
  });

  it('falls back to the reference size for invalid bounds', () => {
    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 10, y: 20 },
        { dx: 5, dy: 6 },
        { bounds: { x: 0, y: 0, width: 0, height: 2160 }, scaleFactor: 1 }
      )
    ).toEqual({
      x: 15,
      y: 26
    });
  });

  it('falls back to scale 1 for invalid scale values', () => {
    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 10, y: 20 },
        { dx: 5, dy: 6 },
        { bounds: { x: 0, y: 0, width: 1920, height: 1080 }, scaleFactor: 0 }
      )
    ).toEqual({
      x: 15,
      y: 26
    });
  });

  it('applies the movement scale to small movement', () => {
    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 100, y: 200 },
        { dx: 48, dy: 0 },
        { bounds: { x: 0, y: 0, width: 1920, height: 1080 }, scaleFactor: 1 },
        { scalePercent: 200 }
      )
    ).toEqual({
      x: 196,
      y: 200
    });
  });

  it('applies the movement scale to medium movement', () => {
    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 100, y: 200 },
        { dx: 128, dy: 0 },
        { bounds: { x: 0, y: 0, width: 1920, height: 1080 }, scaleFactor: 1 },
        { scalePercent: 50 }
      )
    ).toEqual({
      x: 164,
      y: 200
    });
  });

  it('combines display normalization with customized movement scale', () => {
    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 100, y: 200 },
        { dx: 128, dy: 0 },
        { bounds: { x: 0, y: 0, width: 3840, height: 2160 }, scaleFactor: 1 },
        { scalePercent: 150 }
      )
    ).toEqual({
      x: 484,
      y: 200
    });
  });

  it('falls back for invalid display data while applying movement scale', () => {
    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 10, y: 20 },
        { dx: 48, dy: 0 },
        { bounds: { x: 0, y: 0, width: 0, height: 2160 }, scaleFactor: 0 },
        { scalePercent: 200 }
      )
    ).toEqual({
      x: 106,
      y: 20
    });
  });

  it('migrates legacy percentage settings before applying movement scale', () => {
    const legacySettings = {
      percentages: { small: 9, medium: 24, large: 50 }
    } as unknown as PointerMovementSettings;

    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 100, y: 200 },
        { dx: 48, dy: 0 },
        { bounds: { x: 0, y: 0, width: 1920, height: 1080 }, scaleFactor: 1 },
        legacySettings
      )
    ).toEqual({
      x: 196,
      y: 200
    });
  });

  it('normalizes a 4K profile delta once at execution time', () => {
    const profile = createPointerMovementProfile({
      cursor: { x: 100, y: 200 },
      display: {
        bounds: { x: 0, y: 0, width: 3840, height: 2160 },
        scaleFactor: 1
      }
    });

    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 100, y: 200 },
        { dx: profile.recommendedDeltas.medium, dy: 0 },
        { bounds: { x: 0, y: 0, width: 3840, height: 2160 }, scaleFactor: 1 }
      )
    ).toEqual({
      x: 360,
      y: 200
    });
  });

  it('normalizes a high-DPI profile delta once at execution time', () => {
    const profile = createPointerMovementProfile({
      cursor: { x: 100, y: 200 },
      display: {
        bounds: { x: 0, y: 0, width: 3840, height: 2160 },
        scaleFactor: 2
      }
    });

    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 100, y: 200 },
        { dx: profile.recommendedDeltas.medium, dy: 0 },
        { bounds: { x: 0, y: 0, width: 3840, height: 2160 }, scaleFactor: 2 }
      )
    ).toEqual({
      x: 360,
      y: 200
    });
  });

  it('applies configured movement scale during profile delta execution', () => {
    const profile = createPointerMovementProfile({
      cursor: { x: 100, y: 200 },
      display: {
        bounds: { x: 0, y: 0, width: 3840, height: 2160 },
        scaleFactor: 1
      }
    });

    expect(
      calculateDisplayNormalizedMouseTarget(
        { x: 100, y: 200 },
        { dx: profile.recommendedDeltas.medium, dy: 0 },
        { bounds: { x: 0, y: 0, width: 3840, height: 2160 }, scaleFactor: 1 },
        { scalePercent: 150 }
      )
    ).toEqual({
      x: 490,
      y: 200
    });
  });
});

describe('inferPointerMovementSize', () => {
  it('classifies movement deltas by dominant axis', () => {
    expect(inferPointerMovementSize({ dx: 48, dy: 0 })).toBe('small');
    expect(inferPointerMovementSize({ dx: 128, dy: 0 })).toBe('medium');
    expect(inferPointerMovementSize({ dx: 280, dy: 0 })).toBe('large');
    expect(inferPointerMovementSize({ dx: 0, dy: 128 })).toBe('medium');
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

  it('maps Meta to the libnut Windows key', () => {
    expect(toLibnutKeyboardKey('Meta')).toBe('win');
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
