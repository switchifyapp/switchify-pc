import type { PointScanConfig, ScannerColor } from './useScanning';
export type ScanArea = 'point' | 'menu' | 'keyboard';
export type ScanOptions = {
  automatic: boolean;
  intervalMs: number;
  direction: 'forward' | 'reverse';
  passLimit: number;
  pattern: 'grouped' | 'linear';
  color: ScannerColor;
  thickness: 'thin' | 'standard' | 'thick';
};
export type ScanPreferences = Pick<ScanOptions, 'direction' | 'passLimit' | 'pattern' | 'thickness'> & Record<ScanArea, Partial<ScanOptions>>;
export const defaultScanPreferences: ScanPreferences = {
  direction: 'forward', passLimit: 3, pattern: 'grouped', thickness: 'standard',
  point: {}, menu: {}, keyboard: {},
};
export function sharedOptions(config: PointScanConfig): ScanOptions {
  const settings = config.scanPreferences ?? defaultScanPreferences;
  return { automatic: config.automatic, intervalMs: config.blockIntervalMs, color: config.scannerColor,
    direction: settings.direction, passLimit: settings.passLimit, pattern: settings.pattern, thickness: settings.thickness };
}
export function areaOptions(config: PointScanConfig, area: ScanArea): ScanOptions {
  const shared = sharedOptions(config);
  const overrides = (config.scanPreferences ?? defaultScanPreferences)[area];
  return { ...shared, ...Object.fromEntries(Object.entries(overrides).filter(([, value]) => value != null)),
    ...(area === 'point' ? { pattern: 'grouped' as const } : {}) };
}
