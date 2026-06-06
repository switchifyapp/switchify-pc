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

export function cursorOverlayBounds(cursor: CursorPoint, windowSize: number): CursorOverlayBounds {
  const offset = Math.round(windowSize / 2);
  return {
    x: Math.round(cursor.x) - offset,
    y: Math.round(cursor.y) - offset,
    width: windowSize,
    height: windowSize
  };
}
