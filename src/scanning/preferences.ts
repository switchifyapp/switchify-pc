import type { PointScanConfig, ScannerColor } from './useScanning';
export type PanelSize = 'small' | 'medium' | 'large';
export type ScanArea = 'point' | 'menu' | 'keyboard' | 'mouse';
export type ScanOptions = {
  automatic: boolean;
  intervalMs: number;
  direction: 'forward' | 'reverse';
  passLimit: number;
  pattern: 'grouped' | 'linear';
  color: ScannerColor;
  thickness: 'thin' | 'standard' | 'thick';
  /** `standard` is what each scanner has always done. */
  nextScan: 'standard' | 'automatic' | 'wait';
  startFrom: 'standard' | 'beginning' | 'selection';
};
export type ScanPreferences = Pick<ScanOptions, 'direction' | 'passLimit' | 'pattern' | 'thickness'>
  & { panelSize?: PanelSize } & Partial<Pick<ScanOptions, 'nextScan' | 'startFrom'>> & Record<ScanArea, Partial<ScanOptions>>;
export const defaultScanPreferences: ScanPreferences = {
  panelSize: 'medium', direction: 'forward', passLimit: 3, pattern: 'grouped', thickness: 'standard',
  point: {}, menu: {}, keyboard: {}, mouse: {},
};
/** Wait after typing, saved before the choice existed for every scanner, is still in effect. */
export function savedKeyboardWait(config: PointScanConfig) {
  const settings = config.scanPreferences ?? defaultScanPreferences;
  return config.keyboardWaitAfterTyping && settings.keyboard?.nextScan == null && (settings.nextScan ?? 'standard') === 'standard';
}
/** What a scanner does after a selection when nothing has been chosen for it. */
export function usualAfterSelection(config: PointScanConfig, area: ScanArea): Pick<ScanOptions, 'nextScan' | 'startFrom'> {
  return {
    nextScan: area === 'point' || (area === 'keyboard' && savedKeyboardWait(config)) ? 'wait' : 'automatic',
    startFrom: area === 'menu' ? 'selection' : 'beginning',
  };
}
export function sharedOptions(config: PointScanConfig): ScanOptions {
  const settings = config.scanPreferences ?? defaultScanPreferences;
  return { automatic: config.automatic, intervalMs: config.blockIntervalMs, color: config.scannerColor,
    direction: settings.direction, passLimit: settings.passLimit, pattern: settings.pattern, thickness: settings.thickness,
    nextScan: settings.nextScan ?? 'standard', startFrom: settings.startFrom ?? 'standard' };
}
export function areaOptions(config: PointScanConfig, area: ScanArea): ScanOptions {
  const shared = sharedOptions(config);
  const overrides = (config.scanPreferences ?? defaultScanPreferences)[area] ?? {};
  return { ...shared, ...(area === 'mouse' && overrides.automatic == null ? { automatic: areaOptions(config, 'keyboard').automatic } : {}),
    ...Object.fromEntries(Object.entries(overrides).filter(([, value]) => value != null)),
    ...(area === 'point' ? { pattern: 'grouped' as const } : {}) };
}
