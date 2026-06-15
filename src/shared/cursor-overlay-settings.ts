export type CursorOverlaySize = 'small' | 'medium' | 'large';

export type CursorOverlayVisibility = 'onInput' | 'whileControlling';

export type CursorOverlayColor = 'red' | 'green' | 'blue' | 'yellow' | 'white';

export type CursorOverlaySettings = {
  enabled: boolean;
  size: CursorOverlaySize;
  visibility: CursorOverlayVisibility;
  crosshairs: boolean;
  color: CursorOverlayColor;
};

export const DEFAULT_CURSOR_OVERLAY_SETTINGS: CursorOverlaySettings = {
  enabled: true,
  size: 'medium',
  visibility: 'onInput',
  crosshairs: false,
  color: 'red'
};

export const CURSOR_OVERLAY_SIZE_PIXELS: Record<CursorOverlaySize, number> = {
  small: 96,
  medium: 128,
  large: 176
};

export const CURSOR_OVERLAY_COLORS: Record<
  CursorOverlayColor,
  { label: string; rgb: [number, number, number]; hex: string }
> = {
  red: {
    label: 'Red',
    rgb: [211, 47, 47],
    hex: '#d32f2f'
  },
  green: {
    label: 'Green',
    rgb: [132, 255, 145],
    hex: '#84ff91'
  },
  blue: {
    label: 'Blue',
    rgb: [100, 166, 255],
    hex: '#64a6ff'
  },
  yellow: {
    label: 'Yellow',
    rgb: [255, 209, 102],
    hex: '#ffd166'
  },
  white: {
    label: 'White',
    rgb: [255, 255, 255],
    hex: '#ffffff'
  }
};

const cursorOverlaySizes = new Set<CursorOverlaySize>(['small', 'medium', 'large']);
const cursorOverlayVisibilities = new Set<CursorOverlayVisibility>(['onInput', 'whileControlling']);
const cursorOverlayColors = new Set<CursorOverlayColor>(['red', 'green', 'blue', 'yellow', 'white']);

export function resolveCursorOverlaySizePixels(size: CursorOverlaySize): number {
  return CURSOR_OVERLAY_SIZE_PIXELS[size] ?? CURSOR_OVERLAY_SIZE_PIXELS[DEFAULT_CURSOR_OVERLAY_SETTINGS.size];
}

export function resolveCursorOverlayColorRgb(color: CursorOverlayColor): [number, number, number] {
  return CURSOR_OVERLAY_COLORS[color]?.rgb ?? CURSOR_OVERLAY_COLORS[DEFAULT_CURSOR_OVERLAY_SETTINGS.color].rgb;
}

export function normalizeCursorOverlaySettings(value: unknown): CursorOverlaySettings {
  if (!value || typeof value !== 'object') {
    return { ...DEFAULT_CURSOR_OVERLAY_SETTINGS };
  }

  const candidate = value as Partial<CursorOverlaySettings>;
  return {
    enabled:
      typeof candidate.enabled === 'boolean'
        ? candidate.enabled
        : DEFAULT_CURSOR_OVERLAY_SETTINGS.enabled,
    size:
      typeof candidate.size === 'string' && cursorOverlaySizes.has(candidate.size as CursorOverlaySize)
        ? (candidate.size as CursorOverlaySize)
        : DEFAULT_CURSOR_OVERLAY_SETTINGS.size,
    visibility:
      typeof candidate.visibility === 'string' &&
      cursorOverlayVisibilities.has(candidate.visibility as CursorOverlayVisibility)
        ? (candidate.visibility as CursorOverlayVisibility)
        : DEFAULT_CURSOR_OVERLAY_SETTINGS.visibility,
    crosshairs:
      typeof candidate.crosshairs === 'boolean'
        ? candidate.crosshairs
        : DEFAULT_CURSOR_OVERLAY_SETTINGS.crosshairs,
    color:
      typeof candidate.color === 'string' && cursorOverlayColors.has(candidate.color as CursorOverlayColor)
        ? (candidate.color as CursorOverlayColor)
        : DEFAULT_CURSOR_OVERLAY_SETTINGS.color
  };
}
