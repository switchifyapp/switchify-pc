/** Text sizes the main window offers; 0 matches the system text size. */
export const textSizePercents = [0, 100, 125, 150, 175, 200] as const;

export function textSizeLabel(percent: number) {
  return percent === 0 ? "Match system" : `${percent}%`;
}
