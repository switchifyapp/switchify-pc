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
        small: 32,
        medium: 85,
        large: 187
      },
      capabilities: {
        noAckMouseMove: true
      }
    });
  });

  it('returns native target deltas on a 1x display', () => {
    expect(
      createPointerMovementProfile({
        cursor: { x: 100, y: 100 },
        display: {
          bounds: { x: 0, y: 0, width: 1920, height: 1080 },
          scaleFactor: 1
        }
      }).recommendedDeltas
    ).toEqual({
      small: 48,
      medium: 128,
      large: 280
    });
  });

  it('returns small logical deltas on a 3x display', () => {
    expect(
      createPointerMovementProfile({
        cursor: { x: 100, y: 100 },
        display: {
          bounds: { x: 0, y: 0, width: 1920, height: 960 },
          scaleFactor: 3
        }
      }).recommendedDeltas
    ).toEqual({
      small: 16,
      medium: 43,
      large: 93
    });
  });

  it('keeps all recommended deltas below the configured max', () => {
    const profile = createPointerMovementProfile({
      cursor: { x: 100, y: 100 },
      display: {
        bounds: { x: 0, y: 0, width: 3840, height: 2160 },
        scaleFactor: 0.25
      },
      maxDelta: 500
    });

    expect(profile.recommendedDeltas.small).toBeLessThanOrEqual(500);
    expect(profile.recommendedDeltas.medium).toBeLessThanOrEqual(500);
    expect(profile.recommendedDeltas.large).toBeLessThanOrEqual(500);
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
