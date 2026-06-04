import { mkdir, readFile, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';
import { randomUUID } from 'node:crypto';

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
      throw error;
    }
  }

  async save(state: PairingState): Promise<void> {
    await mkdir(dirname(this.filePath), { recursive: true });
    await writeFile(this.filePath, `${JSON.stringify(state, null, 2)}\n`, 'utf8');
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

export function upsertPairedDevice(state: PairingState, device: PairedDevice): PairingState {
  const pairedDevices = state.pairedDevices.filter((existing) => existing.deviceId !== device.deviceId);
  pairedDevices.push(device);

  return {
    ...state,
    pairedDevices
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

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}
