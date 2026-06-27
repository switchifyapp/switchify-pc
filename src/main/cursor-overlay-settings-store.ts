import { readFileSync } from 'node:fs';
import {
  DEFAULT_CURSOR_OVERLAY_SETTINGS,
  normalizeCursorOverlaySettings,
  type CursorOverlaySettings
} from '../shared/cursor-overlay-settings';
import { writeJsonFileAtomicSync } from './json-file-store';

export class JsonCursorOverlaySettingsStore {
  constructor(private readonly filePath: string) {}

  load(): CursorOverlaySettings {
    try {
      const raw = readFileSync(this.filePath, 'utf8');
      return normalizeCursorOverlaySettings(JSON.parse(raw));
    } catch (error) {
      if (isMissingFileError(error)) {
        return { ...DEFAULT_CURSOR_OVERLAY_SETTINGS };
      }

      console.warn('Switchify cursor overlay settings could not be loaded. Defaults will be used.');
      return { ...DEFAULT_CURSOR_OVERLAY_SETTINGS };
    }
  }

  save(settings: CursorOverlaySettings): CursorOverlaySettings {
    const normalized = normalizeCursorOverlaySettings(settings);
    writeJsonFileAtomicSync(this.filePath, `${JSON.stringify(normalized, null, 2)}\n`);
    return normalized;
  }
}

function isMissingFileError(error: unknown): boolean {
  return (
    error !== null &&
    typeof error === 'object' &&
    'code' in error &&
    (error as { code?: unknown }).code === 'ENOENT'
  );
}
