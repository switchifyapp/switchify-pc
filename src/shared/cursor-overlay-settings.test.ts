import { describe, expect, it } from 'vitest';
import {
  DEFAULT_CURSOR_OVERLAY_SETTINGS,
  normalizeCursorOverlaySettings,
  resolveCursorOverlaySizePixels
} from './cursor-overlay-settings';

describe('normalizeCursorOverlaySettings', () => {
  it('returns defaults for non-object values', () => {
    expect(normalizeCursorOverlaySettings(null)).toEqual(DEFAULT_CURSOR_OVERLAY_SETTINGS);
  });

  it('merges valid partial settings with defaults', () => {
    expect(normalizeCursorOverlaySettings({ size: 'large', crosshairs: true })).toEqual({
      ...DEFAULT_CURSOR_OVERLAY_SETTINGS,
      size: 'large',
      crosshairs: true
    });
  });

  it('rejects invalid enum and non-boolean values', () => {
    expect(
      normalizeCursorOverlaySettings({
        enabled: 'yes',
        size: 'huge',
        visibility: 'forever',
        crosshairs: 'true'
      })
    ).toEqual(DEFAULT_CURSOR_OVERLAY_SETTINGS);
  });
});

describe('resolveCursorOverlaySizePixels', () => {
  it('maps size presets to fixed pixel sizes', () => {
    expect(resolveCursorOverlaySizePixels('small')).toBe(96);
    expect(resolveCursorOverlaySizePixels('medium')).toBe(128);
    expect(resolveCursorOverlaySizePixels('large')).toBe(176);
  });
});
