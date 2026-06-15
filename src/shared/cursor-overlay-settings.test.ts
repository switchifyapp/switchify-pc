import { describe, expect, it } from 'vitest';
import {
  DEFAULT_CURSOR_OVERLAY_SETTINGS,
  normalizeCursorOverlaySettings,
  resolveCursorOverlayColorRgb,
  resolveCursorOverlaySizePixels
} from './cursor-overlay-settings';

describe('normalizeCursorOverlaySettings', () => {
  it('returns defaults for non-object values', () => {
    expect(normalizeCursorOverlaySettings(null)).toEqual(DEFAULT_CURSOR_OVERLAY_SETTINGS);
  });

  it('merges valid partial settings with defaults', () => {
    expect(normalizeCursorOverlaySettings({ size: 'large', crosshairs: true, color: 'green' })).toEqual({
      ...DEFAULT_CURSOR_OVERLAY_SETTINGS,
      size: 'large',
      crosshairs: true,
      color: 'green'
    });
  });

  it('rejects invalid enum and non-boolean values', () => {
    expect(
      normalizeCursorOverlaySettings({
        enabled: 'yes',
        size: 'huge',
        visibility: 'forever',
        crosshairs: 'true',
        color: 'purple'
      })
    ).toEqual(DEFAULT_CURSOR_OVERLAY_SETTINGS);
  });

  it('uses brand red as the default color for old settings files', () => {
    expect(
      normalizeCursorOverlaySettings({
        enabled: true,
        size: 'medium',
        visibility: 'onInput',
        crosshairs: false
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

describe('resolveCursorOverlayColorRgb', () => {
  it('maps color presets to RGB values', () => {
    expect(DEFAULT_CURSOR_OVERLAY_SETTINGS.color).toBe('red');
    expect(resolveCursorOverlayColorRgb('red')).toEqual([211, 47, 47]);
    expect(resolveCursorOverlayColorRgb('green')).toEqual([132, 255, 145]);
    expect(resolveCursorOverlayColorRgb('blue')).toEqual([100, 166, 255]);
    expect(resolveCursorOverlayColorRgb('yellow')).toEqual([255, 209, 102]);
    expect(resolveCursorOverlayColorRgb('white')).toEqual([255, 255, 255]);
  });
});
