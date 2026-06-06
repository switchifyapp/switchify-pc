import { describe, expect, it } from 'vitest';
import { cursorOverlayBounds } from './cursor-overlay-state';

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
