import { describe, expect, it } from 'vitest';
import { settingsHashForSection, settingsSectionFromHash } from './settings-route';

describe('settingsSectionFromHash', () => {
  it('uses general for the base settings route', () => {
    expect(settingsSectionFromHash('#/settings')).toBe('general');
  });

  it('reads the updates section from the settings route', () => {
    expect(settingsSectionFromHash('#/settings/updates')).toBe('updates');
  });

  it('uses general for invalid sections', () => {
    expect(settingsSectionFromHash('#/settings/unknown')).toBe('general');
  });
});

describe('settingsHashForSection', () => {
  it('formats the general settings route', () => {
    expect(settingsHashForSection('general')).toBe('/settings');
  });

  it('formats non-default settings sections', () => {
    expect(settingsHashForSection('updates')).toBe('/settings/updates');
  });
});
