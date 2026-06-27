import { readFile } from 'node:fs/promises';
import { randomUUID } from 'node:crypto';
import type { PairedDeviceView } from '../../shared/server-status';
import { backupCorruptJsonFile, writeJsonFileAtomic } from '../json-file-store';

export type PairedDevice = {
  deviceId: string;
  deviceName: string;
  token: string;
  pairedAt: number;
  lastSeenAt: number | null;
};

export type PairingState = {
  desktopId: string;
  pairedDevices: PairedDevice[];
};

export interface PairingStore {
  load(): Promise<PairingState>;
  save(state: PairingState): Promise<void>;
}

export class JsonPairingStore implements PairingStore {
  constructor(private readonly filePath: string) {}

  async load(): Promise<PairingState> {
    try {
      const raw = await readFile(this.filePath, 'utf8');
      return parsePairingState(JSON.parse(raw));
    } catch (error) {
      if (isMissingFileError(error)) {
        const state = createEmptyPairingState();
        await this.save(state);
        return state;
      }

      if (isCorruptPairingStateError(error)) {
        const backup = await backupCorruptJsonFile(this.filePath);
        console.warn(
          backup.backupPath
            ? 'Switchify pairing state could not be loaded. The corrupt file was backed up and a fresh pairing state will be used.'
            : 'Switchify pairing state could not be loaded. A fresh pairing state will be used.'
        );
        const state = createEmptyPairingState();
        await this.save(state);
        return state;
      }

      throw error;
    }
  }

  async save(state: PairingState): Promise<void> {
    await writeJsonFileAtomic(this.filePath, `${JSON.stringify(state, null, 2)}\n`);
  }
}

export class MemoryPairingStore implements PairingStore {
  private state: PairingState;

  constructor(initialState: PairingState = createEmptyPairingState()) {
    this.state = cloneState(initialState);
  }

  async load(): Promise<PairingState> {
    return cloneState(this.state);
  }

  async save(state: PairingState): Promise<void> {
    this.state = cloneState(state);
  }
}

export function createEmptyPairingState(): PairingState {
  return {
    desktopId: randomUUID(),
    pairedDevices: []
  };
}

export function findPairedDevice(state: PairingState, deviceId: string): PairedDevice | null {
  return state.pairedDevices.find((device) => device.deviceId === deviceId) ?? null;
}

export function toPairedDeviceViews(state: PairingState): PairedDeviceView[] {
  return state.pairedDevices.map((device) => ({
    deviceId: device.deviceId,
    deviceName: device.deviceName,
    pairedAt: device.pairedAt,
    lastSeenAt: device.lastSeenAt
  }));
}

export function upsertPairedDevice(state: PairingState, device: PairedDevice): PairingState {
  const pairedDevices = state.pairedDevices.filter((existing) => existing.deviceId !== device.deviceId);
  pairedDevices.push(device);

  return {
    ...state,
    pairedDevices
  };
}

export function removePairedDevice(state: PairingState, deviceId: string): PairingState {
  return {
    ...state,
    pairedDevices: state.pairedDevices.filter((device) => device.deviceId !== deviceId)
  };
}

function parsePairingState(value: unknown): PairingState {
  if (!isRecord(value) || typeof value.desktopId !== 'string' || !Array.isArray(value.pairedDevices)) {
    throw new Error('Invalid pairing state.');
  }

  return {
    desktopId: value.desktopId,
    pairedDevices: value.pairedDevices.map(parsePairedDevice)
  };
}

function parsePairedDevice(value: unknown): PairedDevice {
  if (
    !isRecord(value) ||
    typeof value.deviceId !== 'string' ||
    typeof value.deviceName !== 'string' ||
    typeof value.token !== 'string' ||
    typeof value.pairedAt !== 'number' ||
    !(typeof value.lastSeenAt === 'number' || value.lastSeenAt === null)
  ) {
    throw new Error('Invalid paired device.');
  }

  return {
    deviceId: value.deviceId,
    deviceName: value.deviceName,
    token: value.token,
    pairedAt: value.pairedAt,
    lastSeenAt: value.lastSeenAt
  };
}

function cloneState(state: PairingState): PairingState {
  return {
    desktopId: state.desktopId,
    pairedDevices: state.pairedDevices.map((device) => ({ ...device }))
  };
}

function isMissingFileError(error: unknown): boolean {
  return isRecord(error) && error.code === 'ENOENT';
}

function isCorruptPairingStateError(error: unknown): boolean {
  return (
    error instanceof SyntaxError ||
    (error instanceof Error && (error.message === 'Invalid pairing state.' || error.message === 'Invalid paired device.'))
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}
