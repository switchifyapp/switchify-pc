import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import {
  DEFAULT_POINTER_MOVEMENT_SETTINGS,
  normalizePointerMovementSettings,
  type PointerMovementSettings
} from '../shared/pointer-movement-settings';

export class JsonPointerMovementSettingsStore {
  constructor(private readonly filePath: string) {}

  load(): PointerMovementSettings {
    try {
      const raw = readFileSync(this.filePath, 'utf8');
      return normalizePointerMovementSettings(JSON.parse(raw));
    } catch (error) {
      if (isMissingFileError(error)) {
        return { multipliers: { ...DEFAULT_POINTER_MOVEMENT_SETTINGS.multipliers } };
      }

      console.warn('Switchify pointer movement settings could not be loaded. Defaults will be used.');
      return { multipliers: { ...DEFAULT_POINTER_MOVEMENT_SETTINGS.multipliers } };
    }
  }

  save(settings: PointerMovementSettings): PointerMovementSettings {
    const normalized = normalizePointerMovementSettings(settings);
    mkdirSync(dirname(this.filePath), { recursive: true });
    writeFileSync(this.filePath, `${JSON.stringify(normalized, null, 2)}\n`, 'utf8');
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
