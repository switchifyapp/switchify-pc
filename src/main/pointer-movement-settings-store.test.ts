import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_POINTER_MOVEMENT_SETTINGS } from '../shared/pointer-movement-settings';
import { JsonPointerMovementSettingsStore } from './pointer-movement-settings-store';

describe('JsonPointerMovementSettingsStore', () => {
  let tempDir: string | null = null;
  let warn: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
  });

  afterEach(() => {
    warn.mockRestore();
    if (tempDir) {
      rmSync(tempDir, { recursive: true, force: true });
      tempDir = null;
    }
  });

  it('loads defaults when the settings file is missing', () => {
    expect(store().load()).toEqual(DEFAULT_POINTER_MOVEMENT_SETTINGS);
  });

  it('loads and normalizes valid settings', () => {
    const settingsFile = settingsPath();
    writeFileSync(settingsFile, JSON.stringify({ multipliers: { small: 75, medium: 123, large: 300 } }), 'utf8');

    expect(new JsonPointerMovementSettingsStore(settingsFile).load()).toEqual({
      multipliers: {
        small: 75,
        medium: 125,
        large: 200
      }
    });
  });

  it('loads defaults and warns when JSON is invalid', () => {
    const settingsFile = settingsPath();
    writeFileSync(settingsFile, '{', 'utf8');

    expect(new JsonPointerMovementSettingsStore(settingsFile).load()).toEqual(DEFAULT_POINTER_MOVEMENT_SETTINGS);
    expect(warn).toHaveBeenCalledWith('Switchify pointer movement settings could not be loaded. Defaults will be used.');
  });

  it('saves normalized JSON', () => {
    const settingsFile = settingsPath();
    const saved = new JsonPointerMovementSettingsStore(settingsFile).save({
      multipliers: {
        small: 10,
        medium: 123,
        large: 250
      }
    });

    expect(saved).toEqual({
      multipliers: {
        small: 50,
        medium: 125,
        large: 200
      }
    });
    expect(JSON.parse(readFileSync(settingsFile, 'utf8'))).toEqual(saved);
  });

  it('creates the parent directory when saving', () => {
    tempDir = mkdtempSync(join(tmpdir(), 'switchify-pointer-movement-'));
    const settingsFile = join(tempDir, 'nested', 'pointer-movement-settings.json');

    new JsonPointerMovementSettingsStore(settingsFile).save(DEFAULT_POINTER_MOVEMENT_SETTINGS);

    expect(JSON.parse(readFileSync(settingsFile, 'utf8'))).toEqual(DEFAULT_POINTER_MOVEMENT_SETTINGS);
  });

  function store(): JsonPointerMovementSettingsStore {
    return new JsonPointerMovementSettingsStore(settingsPath());
  }

  function settingsPath(): string {
    if (!tempDir) {
      tempDir = mkdtempSync(join(tmpdir(), 'switchify-pointer-movement-'));
    }
    return join(tempDir, 'pointer-movement-settings.json');
  }
});
