import { MAX_POINTER_DELTA, type PointerMovementProfile } from '../../shared/protocol';

type Point = { x: number; y: number };
type Bounds = { x: number; y: number; width: number; height: number };

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
  const shorterSide = Math.min(bounds.width, bounds.height);

  return {
    displayId: `${bounds.x}:${bounds.y}:${bounds.width}:${bounds.height}:${scaleFactor}`,
    scaleFactor,
    bounds,
    maxDelta,
    recommendedDeltas: {
      small: clamp(Math.round(shorterSide * 0.07), 40, 90),
      medium: clamp(Math.round(shorterSide * 0.18), 110, 220),
      large: clamp(Math.round(shorterSide * 0.35), 240, 450)
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
