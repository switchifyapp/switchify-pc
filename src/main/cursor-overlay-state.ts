export type CursorPoint = {
  x: number;
  y: number;
};

export type CursorOverlayBounds = {
  x: number;
  y: number;
  width: number;
  height: number;
};

export type CursorDisplay = {
  bounds: CursorOverlayBounds;
  scaleFactor: number;
};

export function cursorOverlayBounds(cursor: CursorPoint, windowSize: number): CursorOverlayBounds {
  const offset = Math.round(windowSize / 2);
  return {
    x: Math.round(cursor.x) - offset,
    y: Math.round(cursor.y) - offset,
    width: windowSize,
    height: windowSize
  };
}

export function nativeCursorToElectronPoint(cursor: CursorPoint, displays: CursorDisplay[]): CursorPoint {
  const display = findDisplayForNativeCursor(cursor, displays);
  if (!display) return cursor;

  const scale = display.scaleFactor > 0 && Number.isFinite(display.scaleFactor) ? display.scaleFactor : 1;
  const nativeBounds = toNativeBounds(display);
  return {
    x: display.bounds.x + (cursor.x - nativeBounds.x) / scale,
    y: display.bounds.y + (cursor.y - nativeBounds.y) / scale
  };
}

function findDisplayForNativeCursor(cursor: CursorPoint, displays: CursorDisplay[]): CursorDisplay | null {
  return displays.find((display) => pointInBounds(cursor, toNativeBounds(display))) ?? null;
}

function toNativeBounds(display: CursorDisplay): CursorOverlayBounds {
  const scale = display.scaleFactor > 0 && Number.isFinite(display.scaleFactor) ? display.scaleFactor : 1;
  return {
    x: display.bounds.x * scale,
    y: display.bounds.y * scale,
    width: display.bounds.width * scale,
    height: display.bounds.height * scale
  };
}

function pointInBounds(point: CursorPoint, bounds: CursorOverlayBounds): boolean {
  return (
    point.x >= bounds.x &&
    point.x < bounds.x + bounds.width &&
    point.y >= bounds.y &&
    point.y < bounds.y + bounds.height
  );
}
