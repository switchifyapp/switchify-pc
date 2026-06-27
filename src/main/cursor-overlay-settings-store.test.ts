import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_CURSOR_OVERLAY_SETTINGS } from '../shared/cursor-overlay-settings';
import { JsonCursorOverlaySettingsStore } from './cursor-overlay-settings-store';

describe('JsonCursorOverlaySettingsStore', () => {
  let tempDir: string | null = null;

  afterEach(() => {
    vi.restoreAllMocks();
    if (tempDir) {
      rmSync(tempDir, { recursive: true, force: true });
      tempDir = null;
    }
  });

  function store(): JsonCursorOverlaySettingsStore {
    tempDir = mkdtempSync(join(tmpdir(), 'switchify-cursor-overlay-'));
    return new JsonCursorOverlaySettingsStore(join(tempDir, 'cursor-overlay-settings.json'));
  }

  it('returns defaults when the file is missing', () => {
    expect(store().load()).toEqual(DEFAULT_CURSOR_OVERLAY_SETTINGS);
  });

  it('loads partial files with defaults', () => {
    const settingsStore = store();
    writeFileSync(join(tempDir!, 'cursor-overlay-settings.json'), JSON.stringify({ size: 'large' }), 'utf8');

    expect(settingsStore.load()).toEqual({
      ...DEFAULT_CURSOR_OVERLAY_SETTINGS,
      size: 'large'
    });
  });

  it('uses defaults for corrupt JSON without throwing', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const settingsStore = store();
    writeFileSync(join(tempDir!, 'cursor-overlay-settings.json'), '{', 'utf8');

    expect(settingsStore.load()).toEqual(DEFAULT_CURSOR_OVERLAY_SETTINGS);
    expect(warn).toHaveBeenCalledWith('Switchify cursor overlay settings could not be loaded. Defaults will be used.');
  });

  it('saves normalized settings', () => {
    const settingsStore = store();
    expect(
      settingsStore.save({
        enabled: false,
        size: 'large',
        visibility: 'whileControlling',
        crosshairs: true,
        color: 'blue'
      })
    ).toEqual({
      enabled: false,
      size: 'large',
      visibility: 'whileControlling',
      crosshairs: true,
      color: 'blue'
    });

    expect(settingsStore.load()).toEqual({
      enabled: false,
      size: 'large',
      visibility: 'whileControlling',
      crosshairs: true,
      color: 'blue'
    });
  });

  it('creates parent directories when saving', () => {
    tempDir = mkdtempSync(join(tmpdir(), 'switchify-cursor-overlay-'));
    const settingsFile = join(tempDir, 'nested', 'cursor-overlay-settings.json');

    new JsonCursorOverlaySettingsStore(settingsFile).save(DEFAULT_CURSOR_OVERLAY_SETTINGS);

    expect(JSON.parse(readFileSync(settingsFile, 'utf8'))).toEqual(DEFAULT_CURSOR_OVERLAY_SETTINGS);
  });

  it('leaves no temp files after saving', () => {
    const settingsStore = store();

    settingsStore.save(DEFAULT_CURSOR_OVERLAY_SETTINGS);

    expect(readdirSync(tempDir!).filter((name) => name.endsWith('.tmp'))).toEqual([]);
  });
});
