import { MAX_POINTER_DELTA, type PointerMovementProfile } from '../../shared/protocol';

type Point = { x: number; y: number };
type Bounds = { x: number; y: number; width: number; height: number };

const TARGET_NATIVE_DELTAS = {
  small: 48,
  medium: 128,
  large: 280
};

export function createPointerMovementProfile(input: {
  cursor: Point;
  display: {
    bounds: Bounds;
    scaleFactor: number;
  };
  maxDelta?: number;
}): PointerMovementProfile {
  const bounds = normalizeBounds(input.display.bounds);
  const scaleFactor =
    Number.isFinite(input.display.scaleFactor) && input.display.scaleFactor > 0 ? input.display.scaleFactor : 1;
  const maxDelta = input.maxDelta ?? MAX_POINTER_DELTA;

  return {
    displayId: `${bounds.x}:${bounds.y}:${bounds.width}:${bounds.height}:${scaleFactor}`,
    scaleFactor,
    bounds,
    maxDelta,
    recommendedDeltas: {
      small: toLogicalDelta(TARGET_NATIVE_DELTAS.small, scaleFactor, maxDelta),
      medium: toLogicalDelta(TARGET_NATIVE_DELTAS.medium, scaleFactor, maxDelta),
      large: toLogicalDelta(TARGET_NATIVE_DELTAS.large, scaleFactor, maxDelta)
    },
    capabilities: {
      noAckMouseMove: true
    }
  };
}

function normalizeBounds(bounds: Bounds): Bounds {
  return {
    x: finiteOr(bounds.x, 0),
    y: finiteOr(bounds.y, 0),
    width: positiveFiniteOr(bounds.width, 1),
    height: positiveFiniteOr(bounds.height, 1)
  };
}

function finiteOr(value: number, fallback: number): number {
  return Number.isFinite(value) ? value : fallback;
}

function positiveFiniteOr(value: number, fallback: number): number {
  return Number.isFinite(value) && value > 0 ? value : fallback;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function toLogicalDelta(nativePixels: number, scaleFactor: number, maxDelta: number): number {
  return clamp(Math.round(nativePixels / scaleFactor), 1, maxDelta);
}
