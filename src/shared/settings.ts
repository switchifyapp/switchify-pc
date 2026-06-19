export type SettingsSectionId = 'general' | 'bluetooth' | 'pointer' | 'updates' | 'savedDevices';

export const SETTINGS_SECTION_IDS: SettingsSectionId[] = [
  'general',
  'bluetooth',
  'pointer',
  'updates',
  'savedDevices'
];

export function isSettingsSectionId(value: unknown): value is SettingsSectionId {
  return typeof value === 'string' && SETTINGS_SECTION_IDS.includes(value as SettingsSectionId);
}
