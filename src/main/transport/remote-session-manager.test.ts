import { describe, expect, it, vi } from 'vitest';
import {
  PROTOCOL_VERSION,
  validateProtocolResponse,
  type MouseMoveCommand,
  type PairingCompleteResponse,
  type DisconnectingCommand,
  type PingCommand
} from '../../shared/protocol';
import { createCommandAuthProof, CommandAuthValidator } from '../pairing/auth';
import { PairingApprovalManager } from '../pairing/pairing-approval-manager';
import { PairingManager } from '../pairing/pairing-manager';
import { MemoryPairingStore } from '../pairing/pairing-store';
import type { RemoteConnection } from './remote-connection';
import { RemoteSessionManager } from './remote-session-manager';

const now = 1_724_000_000_000;
const token = 'shared-token';

describe('RemoteSessionManager', () => {
  it('executes authenticated desktop commands through a fake transport connection', async () => {
    const handled: MouseMoveCommand[] = [];
    const manager = createManager({
      onCommand: (command) => {
        handled.push(command as MouseMoveCommand);
        return { ok: true };
      }
    });
    const connection = createConnection();
    manager.addConnection(connection);
    const command = createMouseMoveCommand();

    await manager.handleMessage(connection.id, JSON.stringify(command));

    expect(sentResponses(connection)).toEqual([expect.objectContaining({ type: 'ack', id: command.id, ok: true })]);
    expect(handled).toEqual([command]);
    expect(manager.getAuthenticatedClients()).toEqual([
      expect.objectContaining({
        id: connection.id,
        deviceId: 'android-1',
        transport: 'bluetooth'
      })
    ]);
  });

  it('acks authenticated heartbeat pings without executing desktop commands', async () => {
    const onCommand = vi.fn();
    const manager = createManager({ onCommand });
    const connection = createConnection();
    manager.addConnection(connection);
    const command = createPingCommand();

    await manager.handleMessage(connection.id, JSON.stringify(command));

    expect(sentResponses(connection)).toEqual([expect.objectContaining({ type: 'ack', id: command.id, ok: true })]);
    expect(onCommand).not.toHaveBeenCalled();
    expect(manager.getAuthenticatedClients()).toEqual([
      expect.objectContaining({
        id: connection.id,
        deviceId: 'android-1',
        transport: 'bluetooth'
      })
    ]);
  });

  it('rejects unauthenticated heartbeat pings with a structured auth error', async () => {
    const onCommand = vi.fn();
    const manager = createManager({ onCommand });
    const connection = createConnection();
    manager.addConnection(connection);

    await manager.handleMessage(connection.id, JSON.stringify(createPingCommand({ auth: 'invalid' })));

    expect(sentResponses(connection)).toEqual([
      expect.objectContaining({
        type: 'error',
        ok: false,
        error: expect.objectContaining({ code: 'invalid_auth' })
      })
    ]);
    expect(onCommand).not.toHaveBeenCalled();
  });

  it('keeps pairing requests pending until they are accepted', async () => {
    const store = new MemoryPairingStore({ desktopId: 'desktop-1', pairedDevices: [] });
    const approvalManager = new PairingApprovalManager(store, () => now);
    const manager = createManager({
      store,
      pairingApprovalManager: approvalManager,
      authValidator: new CommandAuthValidator(store, () => now)
    });
    const connection = createConnection();
    manager.addConnection(connection);

    await manager.handleMessage(connection.id, JSON.stringify(createPairingRequest()));

    expect(sentResponses(connection)).toEqual([]);
    expect(manager.getPendingPairingRequests()).toEqual([
      expect.objectContaining({
        requestId: 'approval-1',
        deviceName: 'Android smoke',
        verificationCode: expect.stringMatching(/^\d{6}$/)
      })
    ]);

    await expect(manager.respondToPairingRequest('approval-1', 'accept')).resolves.toEqual({ ok: true });
    const [response] = sentResponses(connection) as PairingCompleteResponse[];

    expect(response).toMatchObject({
      type: 'pairing.complete',
      ok: true,
      payload: {
        desktopId: 'desktop-1',
        deviceId: 'android-smoke-1'
      }
    });
    expect(response.payload.token).not.toBe('');
    expect(manager.getAuthenticatedClientCount()).toBe(1);
  });

  it('returns structured errors for malformed JSON without crashing', async () => {
    const manager = createManager();
    const connection = createConnection();
    manager.addConnection(connection);

    await manager.handleMessage(connection.id, '{');

    expect(sentResponses(connection)).toEqual([
      expect.objectContaining({
        type: 'error',
        ok: false,
        error: expect.objectContaining({ code: 'invalid_json' })
      })
    ]);
  });

  it('acks authenticated client disconnect intent without executing desktop commands', async () => {
    const onClientDisconnecting = vi.fn();
    const onCommand = vi.fn();
    const manager = createManager({ onClientDisconnecting, onCommand });
    const connection = createConnection();
    manager.addConnection(connection);
    const command = createDisconnectingCommand();

    await manager.handleMessage(connection.id, JSON.stringify(command));

    expect(sentResponses(connection)).toEqual([expect.objectContaining({ type: 'ack', id: command.id, ok: true })]);
    expect(onClientDisconnecting).toHaveBeenCalledWith(connection.id, 'android-1');
    expect(onCommand).not.toHaveBeenCalled();
    expect(manager.getAuthenticatedClients()).toEqual([
      expect.objectContaining({
        id: connection.id,
        deviceId: 'android-1',
        transport: 'bluetooth'
      })
    ]);
  });
});

function createManager(
  overrides: Partial<ConstructorParameters<typeof RemoteSessionManager>[0]> & { store?: MemoryPairingStore } = {}
): RemoteSessionManager {
  const store =
    overrides.store ??
    new MemoryPairingStore({
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

  return new RemoteSessionManager({
    pairingManager: new PairingManager(store),
    authValidator: new CommandAuthValidator(store, () => now),
    ...overrides
  });
}

function createConnection(): RemoteConnection {
  return {
    id: 'connection-1',
    kind: 'bluetooth',
    label: 'Bluetooth test connection',
    remoteAddress: null,
    send: vi.fn(),
    close: vi.fn()
  };
}

function sentResponses(connection: RemoteConnection): unknown[] {
  return vi.mocked(connection.send).mock.calls.map(([message]) => {
    const parsed = JSON.parse(message);
    expect(validateProtocolResponse(parsed)).toMatchObject({ ok: true });
    return parsed;
  });
}

function createPingCommand(overrides: Partial<PingCommand> = {}): PingCommand {
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

function createMouseMoveCommand(overrides: Partial<MouseMoveCommand> = {}): MouseMoveCommand {
  const command = {
    version: PROTOCOL_VERSION,
    id: 'move-1',
    deviceId: 'android-1',
    timestamp: now,
    type: 'mouse.move',
    payload: { dx: 5, dy: -3 },
    auth: ''
  } satisfies MouseMoveCommand;
  const merged = { ...command, ...overrides } as MouseMoveCommand;
  return {
    ...merged,
    auth: overrides.auth ?? createCommandAuthProof(merged, token)
  };
}

function createDisconnectingCommand(overrides: Partial<DisconnectingCommand> = {}): DisconnectingCommand {
  const command = {
    version: PROTOCOL_VERSION,
    id: 'disconnecting-1',
    deviceId: 'android-1',
    timestamp: now,
    type: 'connection.disconnecting',
    payload: {},
    auth: ''
  } satisfies DisconnectingCommand;
  const merged = { ...command, ...overrides } as DisconnectingCommand;
  return {
    ...merged,
    auth: overrides.auth ?? createCommandAuthProof(merged, token)
  };
}

function createPairingRequest() {
  return {
    version: PROTOCOL_VERSION,
    id: 'approval-1',
    type: 'pairing.request',
    payload: {
      deviceId: 'android-smoke-1',
      deviceName: 'Android smoke',
      desktopId: 'desktop-1',
      requestNonce: 'nonce'
    }
  };
}

