export type CursorOverlaySize = 'small' | 'medium' | 'large';

export type CursorOverlayVisibility = 'onInput' | 'whileControlling';

export type CursorOverlaySettings = {
  enabled: boolean;
  size: CursorOverlaySize;
  visibility: CursorOverlayVisibility;
  crosshairs: boolean;
};

export const DEFAULT_CURSOR_OVERLAY_SETTINGS: CursorOverlaySettings = {
  enabled: true,
  size: 'medium',
  visibility: 'onInput',
  crosshairs: false
};

export const CURSOR_OVERLAY_SIZE_PIXELS: Record<CursorOverlaySize, number> = {
  small: 96,
  medium: 128,
  large: 176
};

const cursorOverlaySizes = new Set<CursorOverlaySize>(['small', 'medium', 'large']);
const cursorOverlayVisibilities = new Set<CursorOverlayVisibility>(['onInput', 'whileControlling']);

export function resolveCursorOverlaySizePixels(size: CursorOverlaySize): number {
  return CURSOR_OVERLAY_SIZE_PIXELS[size] ?? CURSOR_OVERLAY_SIZE_PIXELS[DEFAULT_CURSOR_OVERLAY_SETTINGS.size];
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
        : DEFAULT_CURSOR_OVERLAY_SETTINGS.crosshairs
  };
}
