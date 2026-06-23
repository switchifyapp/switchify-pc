import { describe, expect, it } from 'vitest';
import { MAX_POINTER_DELTA } from '../../shared/protocol';
import { createPointerMovementProfile } from './pointer-profile';

describe('createPointerMovementProfile', () => {
  it('creates baseline deltas for a 1280x720 display at 1.5 scale', () => {
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
    expect(profile.capabilities.noAckCommands).toContain('keyboard.textStream.char');
    expect(profile.capabilities.noAckCommands).toContain('keyboard.textStream.key');
    expect(profile.capabilities.supportedCommands).toEqual(
      expect.arrayContaining([
        'keyboard.textStream.open',
        'keyboard.textStream.chunk',
        'keyboard.textStream.char',
        'keyboard.textStream.key',
        'keyboard.textStream.close'
      ])
    );
  });

  it('returns 1080p baseline deltas on a 1920x1080 display at 1x scale', () => {
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

  it('applies pointer movement multipliers to recommended deltas', () => {
    expect(
      createPointerMovementProfile({
        cursor: { x: 100, y: 100 },
        display: {
          bounds: { x: 0, y: 0, width: 1920, height: 1080 },
          scaleFactor: 1
        },
        movementSettings: {
          multipliers: {
            small: 50,
            medium: 125,
            large: 200
          }
        }
      }).recommendedDeltas
    ).toEqual({
      small: 24,
      medium: 160,
      large: 500
    });
  });

  it('divides customized deltas by scale factor on high-DPI displays', () => {
    expect(
      createPointerMovementProfile({
        cursor: { x: 100, y: 100 },
        display: {
          bounds: { x: 0, y: 0, width: 3840, height: 2160 },
          scaleFactor: 2
        },
        movementSettings: {
          multipliers: {
            small: 100,
            medium: 150,
            large: 100
          }
        }
      }).recommendedDeltas.medium
    ).toBe(96);
  });

  it('normalizes invalid movement settings to defaults', () => {
    expect(
      createPointerMovementProfile({
        cursor: { x: 100, y: 100 },
        display: {
          bounds: { x: 0, y: 0, width: 1920, height: 1080 },
          scaleFactor: 1
        },
        movementSettings: {
          multipliers: {
            small: Number.NaN,
            medium: Number.POSITIVE_INFINITY,
            large: 0
          }
        }
      }).recommendedDeltas
    ).toEqual({
      small: 48,
      medium: 128,
      large: 140
    });
  });

  it('keeps profile deltas stable on a 4K display at 1x scale', () => {
    expect(
      createPointerMovementProfile({
        cursor: { x: 100, y: 100 },
        display: {
          bounds: { x: 0, y: 0, width: 3840, height: 2160 },
          scaleFactor: 1
        }
      }).recommendedDeltas
    ).toEqual({
      small: 48,
      medium: 128,
      large: 280
    });
  });

  it('keeps profile deltas stable on ultrawide displays', () => {
    expect(
      createPointerMovementProfile({
        cursor: { x: 100, y: 100 },
        display: {
          bounds: { x: 0, y: 0, width: 3440, height: 1440 },
          scaleFactor: 1
        }
      }).recommendedDeltas
    ).toEqual({
      small: 48,
      medium: 128,
      large: 280
    });
  });

  it('still divides by scale factor on high-DPI displays', () => {
    expect(
      createPointerMovementProfile({
        cursor: { x: 100, y: 100 },
        display: {
          bounds: { x: 0, y: 0, width: 3840, height: 2160 },
          scaleFactor: 2
        }
      }).recommendedDeltas
    ).toEqual({
      small: 24,
      medium: 64,
      large: 140
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
