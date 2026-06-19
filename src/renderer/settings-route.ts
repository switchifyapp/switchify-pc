import type { SettingsSectionId } from '../shared/settings';
import { isSettingsSectionId } from '../shared/settings';

export function settingsHashForSection(section: SettingsSectionId): string {
  return section === 'general' ? '/settings' : `/settings/${section}`;
}

export function settingsSectionFromHash(hash: string): SettingsSectionId {
  const section = hash.replace(/^#\/settings\/?/, '');
  return isSettingsSectionId(section) ? section : 'general';
}
