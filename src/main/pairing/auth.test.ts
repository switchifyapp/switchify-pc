import { describe, expect, it } from 'vitest';
import type { MouseClickCommand, MouseMoveCommand, PingCommand } from '../../shared/protocol';
import { PROTOCOL_VERSION } from '../../shared/protocol';
import { createCommandAuthProof, CommandAuthValidator, COMMAND_TIMESTAMP_TOLERANCE_MS } from './auth';
import { MemoryPairingStore } from './pairing-store';

const token = 'shared-token';
const now = 1_724_000_000_000;

function createStore(): MemoryPairingStore {
  return new MemoryPairingStore({
    desktopId: 'desktop-1',
    pairedDevices: [
      {
        deviceId: 'android-1',
        deviceName: 'Android device',
        token,
        pairedAt: now - 1_000,
        lastSeenAt: null
      }
    ]
  });
}

function createCommand(overrides: Partial<PingCommand> = {}): PingCommand {
  const command = {
    version: PROTOCOL_VERSION,
    id: 'request-1',
    deviceId: 'android-1',
    timestamp: now,
    type: 'connection.ping',
    payload: {},
    auth: ''
  } satisfies PingCommand;

  const merged = { ...command, ...overrides } as PingCommand;
  return {
    ...merged,
    auth: overrides.auth ?? createCommandAuthProof(merged, token)
  };
}

describe('CommandAuthValidator', () => {
  it('accepts commands from paired devices with a valid auth proof', async () => {
    const store = createStore();
    const validator = new CommandAuthValidator(store, () => now);
    const command = createCommand();

    await expect(validator.validate(command)).resolves.toMatchObject({ ok: true, command });
    expect((await store.load()).pairedDevices[0].lastSeenAt).toBe(now);
  });

  it('rejects unknown devices before auth succeeds', async () => {
    const validator = new CommandAuthValidator(createStore(), () => now);

    await expect(validator.validate(createCommand({ deviceId: 'android-unknown' }))).resolves.toEqual({
      ok: false,
      reason: 'unknown_device'
    });
  });

  it('rejects invalid auth proofs', async () => {
    const store = createStore();
    const validator = new CommandAuthValidator(store, () => now);

    await expect(validator.validate(createCommand({ auth: 'bad-proof' }))).resolves.toEqual({
      ok: false,
      reason: 'invalid_auth'
    });
    expect((await store.load()).pairedDevices[0].lastSeenAt).toBeNull();
  });

  it('canonicalizes missing response mode as ack', () => {
    const implicitAck = createCommand();
    const explicitAck = createCommand({ responseMode: 'ack' });

    expect(createCommandAuthProof(implicitAck, token)).toBe(createCommandAuthProof(explicitAck, token));
  });

  it('includes no-response mode in command auth proofs', async () => {
    const ackMove = createMouseMoveCommand();
    const noAckMove = createMouseMoveCommand({ responseMode: 'none' });
    const ackClick = createMouseClickCommand();
    const noAckClick = createMouseClickCommand({ responseMode: 'none' });

    expect(createCommandAuthProof(noAckMove, token)).not.toBe(createCommandAuthProof(ackMove, token));
    expect(createCommandAuthProof(noAckClick, token)).not.toBe(createCommandAuthProof(ackClick, token));
  });

  it('rejects response mode tampering after signing', async () => {
    const store = createStore();
    const validator = new CommandAuthValidator(store, () => now);
    const command = createMouseMoveCommand();

    await expect(validator.validate({ ...command, responseMode: 'none' })).resolves.toEqual({
      ok: false,
      reason: 'invalid_auth'
    });

    const clickCommand = createMouseClickCommand();
    await expect(validator.validate({ ...clickCommand, responseMode: 'none' })).resolves.toEqual({
      ok: false,
      reason: 'invalid_auth'
    });
  });

  it('rejects expired timestamps', async () => {
    const store = createStore();
    const validator = new CommandAuthValidator(store, () => now);

    await expect(
      validator.validate(createCommand({ timestamp: now - COMMAND_TIMESTAMP_TOLERANCE_MS - 1 }))
    ).resolves.toEqual({
      ok: false,
      reason: 'expired_timestamp'
    });
    expect((await store.load()).pairedDevices[0].lastSeenAt).toBeNull();
  });

  it('rejects duplicate request ids inside the replay cache window', async () => {
    const validator = new CommandAuthValidator(createStore(), () => now);
    const command = createCommand();

    await expect(validator.validate(command)).resolves.toMatchObject({ ok: true });
    await expect(validator.validate(command)).resolves.toEqual({
      ok: false,
      reason: 'duplicate_request'
    });
  });

  it('rejects oversized payloads through protocol validation', async () => {
    const validator = new CommandAuthValidator(createStore(), () => now);

    await expect(
      validator.validate({
        ...createCommand(),
        type: 'keyboard.typeText',
        payload: { text: 'x'.repeat(2_001) }
      })
    ).resolves.toEqual({
      ok: false,
      reason: 'invalid_payload'
    });
  });
});

function createMouseMoveCommand(overrides: Partial<MouseMoveCommand> = {}): MouseMoveCommand {
  const command = {
    version: PROTOCOL_VERSION,
    id: 'move-1',
    deviceId: 'android-1',
    timestamp: now,
    type: 'mouse.move',
    payload: { dx: 12, dy: -6 },
    auth: ''
  } satisfies MouseMoveCommand;

  const merged = { ...command, ...overrides } as MouseMoveCommand;
  return {
    ...merged,
    auth: overrides.auth ?? createCommandAuthProof(merged, token)
  };
}

function createMouseClickCommand(overrides: Partial<MouseClickCommand> = {}): MouseClickCommand {
  const command = {
    version: PROTOCOL_VERSION,
    id: 'click-1',
    deviceId: 'android-1',
    timestamp: now,
    type: 'mouse.click',
    payload: { button: 'left' },
    auth: ''
  } satisfies MouseClickCommand;

  const merged = { ...command, ...overrides } as MouseClickCommand;
  return {
    ...merged,
    auth: overrides.auth ?? createCommandAuthProof(merged, token)
  };
}
