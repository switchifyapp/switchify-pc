import { describe, expect, it } from 'vitest';
import { cursorOverlayBounds, nativeCursorToElectronPoint } from './cursor-overlay-state';

describe('cursorOverlayBounds', () => {
  it('centers the overlay window on the cursor', () => {
    expect(cursorOverlayBounds({ x: 100, y: 200 }, 96)).toEqual({
      x: 52,
      y: 152,
      width: 96,
      height: 96
    });
  });

  it('supports negative monitor coordinates', () => {
    expect(cursorOverlayBounds({ x: -300, y: 100 }, 96)).toEqual({
      x: -348,
      y: 52,
      width: 96,
      height: 96
    });
  });
});

describe('nativeCursorToElectronPoint', () => {
  it('converts native cursor coordinates on a scaled display', () => {
    expect(
      nativeCursorToElectronPoint(
        { x: 900, y: 600 },
        [{ bounds: { x: 0, y: 0, width: 640, height: 360 }, scaleFactor: 3 }]
      )
    ).toEqual({ x: 300, y: 200 });
  });

  it('converts native cursor coordinates on a negative scaled display', () => {
    expect(
      nativeCursorToElectronPoint(
        { x: -450, y: 300 },
        [{ bounds: { x: -640, y: 0, width: 640, height: 360 }, scaleFactor: 1.5 }]
      )
    ).toEqual({ x: -300, y: 200 });
  });

  it('returns the original cursor point when no display contains it', () => {
    expect(
      nativeCursorToElectronPoint(
        { x: 2500, y: 1200 },
        [{ bounds: { x: 0, y: 0, width: 640, height: 360 }, scaleFactor: 3 }]
      )
    ).toEqual({ x: 2500, y: 1200 });
  });
});
