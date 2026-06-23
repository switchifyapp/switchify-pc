import { MAX_POINTER_DELTA, NO_ACK_CONTROL_COMMAND_TYPES, type PointerMovementProfile } from '../../shared/protocol';
import {
  normalizePointerMovementSettings,
  pointerMovementFractionFor,
  type PointerMovementSettings
} from '../../shared/pointer-movement-settings';

type Point = { x: number; y: number };
type Bounds = { x: number; y: number; width: number; height: number };

export function createPointerMovementProfile(input: {
  cursor: Point;
  display: {
    bounds: Bounds;
    scaleFactor: number;
  };
  movementSettings?: PointerMovementSettings;
  maxDelta?: number;
}): PointerMovementProfile {
  const bounds = normalizeBounds(input.display.bounds);
  const scaleFactor =
    Number.isFinite(input.display.scaleFactor) && input.display.scaleFactor > 0 ? input.display.scaleFactor : 1;
  const maxDelta = input.maxDelta ?? MAX_POINTER_DELTA;
  const movementSettings = normalizePointerMovementSettings(input.movementSettings);
  const referenceSize = Math.min(bounds.width, bounds.height);
  const targetNativeDeltas = {
    small: referenceSize * pointerMovementFractionFor(movementSettings, 'small'),
    medium: referenceSize * pointerMovementFractionFor(movementSettings, 'medium'),
    large: referenceSize * pointerMovementFractionFor(movementSettings, 'large')
  };

  return {
    displayId: `${bounds.x}:${bounds.y}:${bounds.width}:${bounds.height}:${scaleFactor}`,
    scaleFactor,
    bounds,
    maxDelta,
    recommendedDeltas: {
      small: toLogicalDelta(targetNativeDeltas.small, scaleFactor, maxDelta),
      medium: toLogicalDelta(targetNativeDeltas.medium, scaleFactor, maxDelta),
      large: toLogicalDelta(targetNativeDeltas.large, scaleFactor, maxDelta)
    },
    capabilities: {
      noAckMouseMove: true,
      noAckCommands: [...NO_ACK_CONTROL_COMMAND_TYPES],
      supportedCommands: [
        ...NO_ACK_CONTROL_COMMAND_TYPES,
        'keyboard.textStream.open',
        'keyboard.textStream.chunk',
        'keyboard.textStream.close',
        'connection.ping',
        'pointer.profile'
      ]
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
