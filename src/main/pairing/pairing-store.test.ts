import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { JsonPairingStore, removePairedDevice, toPairedDeviceViews, type PairingState } from './pairing-store';

describe('toPairedDeviceViews', () => {
  it('removes shared tokens from paired device metadata', () => {
    const state = {
      desktopId: 'desktop-1',
      pairedDevices: [
        {
          deviceId: 'android-1',
          deviceName: 'Android device',
          token: 'secret-token',
          pairedAt: 1_000,
          lastSeenAt: 2_000
        }
      ]
    } satisfies PairingState;

    const views = toPairedDeviceViews(state);

    expect(views).toEqual([
      {
        deviceId: 'android-1',
        deviceName: 'Android device',
        pairedAt: 1_000,
        lastSeenAt: 2_000
      }
    ]);
    expect(JSON.stringify(views)).not.toContain('secret-token');
  });
});

describe('removePairedDevice', () => {
  it('removes only the matching paired device and preserves desktop id', () => {
    const state = createState();

    const nextState = removePairedDevice(state, 'android-1');

    expect(nextState.desktopId).toBe('desktop-1');
    expect(nextState.pairedDevices).toEqual([
      {
        deviceId: 'android-2',
        deviceName: 'Tablet',
        token: 'secret-token-2',
        pairedAt: 3_000,
        lastSeenAt: null
      }
    ]);
    expect(state.pairedDevices).toHaveLength(2);
  });

  it('leaves paired devices unchanged when the device id is missing', () => {
    const state = createState();

    const nextState = removePairedDevice(state, 'missing');

    expect(nextState).toEqual(state);
    expect(nextState).not.toBe(state);
    expect(nextState.pairedDevices).not.toBe(state.pairedDevices);
  });
});

describe('JsonPairingStore', () => {
  let tempDir: string | null = null;

  afterEach(() => {
    vi.restoreAllMocks();
    if (tempDir) {
      rmSync(tempDir, { recursive: true, force: true });
      tempDir = null;
    }
  });

  it('creates and saves fresh pairing state when the file is missing', async () => {
    const filePath = pairingPath();

    const state = await new JsonPairingStore(filePath).load();

    expect(state.desktopId).toMatch(/[0-9a-f-]{36}/);
    expect(state.pairedDevices).toEqual([]);
    expect(JSON.parse(readFileSync(filePath, 'utf8'))).toEqual(state);
  });

  it('loads valid pairing state', async () => {
    const filePath = pairingPath();
    writeFileSync(filePath, JSON.stringify(createState()), 'utf8');

    await expect(new JsonPairingStore(filePath).load()).resolves.toEqual(createState());
  });

  it('saves formatted JSON and creates parent directories', async () => {
    const filePath = nestedPairingPath();

    await new JsonPairingStore(filePath).save(createState());

    expect(readFileSync(filePath, 'utf8')).toBe(`${JSON.stringify(createState(), null, 2)}\n`);
  });

  it('leaves no temp files after a successful save', async () => {
    const filePath = pairingPath();

    await new JsonPairingStore(filePath).save(createState());

    expect(readdirSync(tempDir!).filter((name) => name.endsWith('.tmp'))).toEqual([]);
  });

  it('backs up invalid JSON and replaces it with fresh valid state', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const filePath = pairingPath();
    writeFileSync(filePath, '{', 'utf8');

    const state = await new JsonPairingStore(filePath).load();

    expect(state.pairedDevices).toEqual([]);
    expect(JSON.parse(readFileSync(filePath, 'utf8'))).toEqual(state);
    expect(corruptBackups()).toHaveLength(1);
    expect(warn.mock.calls.flat().join('\n')).not.toContain('{');
  });

  it('backs up NUL-byte files and replaces them with fresh valid state', async () => {
    const filePath = pairingPath();
    writeFileSync(filePath, '\0'.repeat(562), 'utf8');

    const state = await new JsonPairingStore(filePath).load();

    expect(state.desktopId).toMatch(/[0-9a-f-]{36}/);
    expect(state.pairedDevices).toEqual([]);
    expect(readFileSync(filePath, 'utf8')).toContain('"pairedDevices": []');
    expect(corruptBackups()).toHaveLength(1);
  });

  it('backs up invalid pairing state schema and replaces it with fresh valid state', async () => {
    const filePath = pairingPath();
    writeFileSync(filePath, JSON.stringify({ desktopId: 1, pairedDevices: [] }), 'utf8');

    const state = await new JsonPairingStore(filePath).load();

    expect(state.pairedDevices).toEqual([]);
    expect(corruptBackups()).toHaveLength(1);
  });

  it('backs up invalid paired-device entries and replaces them with fresh valid state', async () => {
    const filePath = pairingPath();
    writeFileSync(
      filePath,
      JSON.stringify({
        desktopId: 'desktop-1',
        pairedDevices: [{ deviceId: 'android-1', token: 'secret-token' }]
      }),
      'utf8'
    );

    const state = await new JsonPairingStore(filePath).load();

    expect(state.pairedDevices).toEqual([]);
    expect(corruptBackups()).toHaveLength(1);
  });

  it('does not log tokens or corrupt pairing contents during recovery', async () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);
    const filePath = pairingPath();
    writeFileSync(
      filePath,
      JSON.stringify({
        desktopId: 'desktop-1',
        pairedDevices: [{ deviceId: 'android-1', token: 'secret-token' }]
      }),
      'utf8'
    );

    await new JsonPairingStore(filePath).load();

    const warningText = warn.mock.calls.flat().join('\n');
    expect(warningText).not.toContain('secret-token');
    expect(warningText).not.toContain('android-1');
  });

  function pairingPath(): string {
    if (!tempDir) {
      tempDir = mkdtempSync(join(tmpdir(), 'switchify-pairing-store-'));
    }

    return join(tempDir, 'pairing-state.json');
  }

  function nestedPairingPath(): string {
    if (!tempDir) {
      tempDir = mkdtempSync(join(tmpdir(), 'switchify-pairing-store-'));
    }

    return join(tempDir, 'nested', 'pairing-state.json');
  }

  function corruptBackups(): string[] {
    return readdirSync(tempDir!).filter((name) => name.startsWith('pairing-state.corrupt-'));
  }
});

function createState(): PairingState {
  return {
    desktopId: 'desktop-1',
    pairedDevices: [
      {
        deviceId: 'android-1',
        deviceName: 'Phone',
        token: 'secret-token-1',
        pairedAt: 1_000,
        lastSeenAt: 2_000
      },
      {
        deviceId: 'android-2',
        deviceName: 'Tablet',
        token: 'secret-token-2',
        pairedAt: 3_000,
        lastSeenAt: null
      }
    ]
  };
}
