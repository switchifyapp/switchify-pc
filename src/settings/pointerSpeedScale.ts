const minSpeed = 5;
const normalSpeed = 100;
const maxSpeed = 1350;

export function speedLevel(percent: number): number {
  if (percent <= normalSpeed) {
    return 1 + 4 * Math.log(Math.max(minSpeed, percent) / minSpeed) / Math.log(normalSpeed / minSpeed);
  }
  return 5 + 5 * Math.log(Math.min(maxSpeed, percent) / normalSpeed) / Math.log(maxSpeed / normalSpeed);
}

export function speedPercent(level: number): number {
  const bounded = Math.min(10, Math.max(1, level));
  const value = bounded <= 5
    ? minSpeed * (normalSpeed / minSpeed) ** ((bounded - 1) / 4)
    : normalSpeed * (maxSpeed / normalSpeed) ** ((bounded - 5) / 5);
  return Math.min(maxSpeed, Math.max(minSpeed, Math.round(value / 5) * 5));
}

export function speedLevelLabel(percent: number): string {
  return speedLevel(percent).toFixed(1);
}
