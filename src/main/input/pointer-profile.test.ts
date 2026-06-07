import { describe, expect, it } from 'vitest';
import { MAX_POINTER_DELTA } from '../../shared/protocol';
import { createPointerMovementProfile } from './pointer-profile';

describe('createPointerMovementProfile', () => {
  it('creates sensible deltas for a 1280x720 display at 1.5 scale', () => {
    const profile = createPointerMovementProfile({
      cursor: { x: 100, y: 100 },
      display: {
        bounds: { x: 0, y: 0, width: 1280, height: 720 },
        scaleFactor: 1.5
      }
    });

    expect(profile).toMatchObject({
      displayId: '0:0:1280:720:1.5',
      scaleFactor: 1.5,
      bounds: { x: 0, y: 0, width: 1280, height: 720 },
      maxDelta: MAX_POINTER_DELTA,
      recommendedDeltas: {
        small: 50,
        medium: 130,
        large: 252
      }
    });
  });

  it('creates larger deltas for a 1920x1080 display', () => {
    expect(
      createPointerMovementProfile({
        cursor: { x: 100, y: 100 },
        display: {
          bounds: { x: 0, y: 0, width: 1920, height: 1080 },
          scaleFactor: 1
        }
      }).recommendedDeltas
    ).toEqual({
      small: 76,
      medium: 194,
      large: 378
    });
  });

  it('clamps large deltas on very large displays', () => {
    expect(
      createPointerMovementProfile({
        cursor: { x: 100, y: 100 },
        display: {
          bounds: { x: 0, y: 0, width: 3840, height: 2160 },
          scaleFactor: 1
        }
      }).recommendedDeltas.large
    ).toBe(450);
  });

  it('falls back to scale factor 1 for invalid scale values', () => {
    const profile = createPointerMovementProfile({
      cursor: { x: 100, y: 100 },
      display: {
        bounds: { x: 0, y: 0, width: 1280, height: 720 },
        scaleFactor: 0
      }
    });

    expect(profile.scaleFactor).toBe(1);
    expect(profile.displayId).toBe('0:0:1280:720:1');
  });

  it('uses negative display coordinates in the stable display id', () => {
    const profile = createPointerMovementProfile({
      cursor: { x: -100, y: 100 },
      display: {
        bounds: { x: -1920, y: 0, width: 1920, height: 1080 },
        scaleFactor: 1.25
      }
    });

    expect(profile.displayId).toBe('-1920:0:1920:1080:1.25');
  });
});
