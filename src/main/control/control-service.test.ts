import { describe, expect, it, vi } from 'vitest';
import { DEFAULT_BLUETOOTH_STATUS } from '../../shared/bluetooth-status';
import {
  PROTOCOL_VERSION,
  validateProtocolResponse,
  type PairingCompleteResponse,
  type PingCommand
} from '../../shared/protocol';
import { createCommandAuthProof, CommandAuthValidator } from '../pairing/auth';
import { PairingApprovalManager } from '../pairing/pairing-approval-manager';
import { PairingManager } from '../pairing/pairing-manager';
import { MemoryPairingStore } from '../pairing/pairing-store';
import type { RemoteConnection } from '../transport/remote-connection';
import { ControlService } from './control-service';

const now = 1_724_000_000_000;
const token = 'shared-token';

describe('ControlService', () => {
  it('tracks authenticated Bluetooth connections in control status', async () => {
    const service = createControlService();
    const connection = createConnection();
    service.addRemoteConnection(connection);

    await service.handleRemoteMessage(connection.id, JSON.stringify(createPingCommand()));

    expect(sentResponses(connection)).toEqual([expect.objectContaining({ type: 'ack', id: 'request-1', ok: true })]);
    expect(service.getStatus()).toMatchObject({
      connectedClientCount: 1,
      connectedClients: [
        {
          id: connection.id,
          deviceId: 'android-1',
          transport: 'bluetooth'
        }
      ]
    });
  });

  it('disconnects all active remote connections', async () => {
    const service = createControlService();
    const connection = createConnection();
    service.addRemoteConnection(connection);
    await service.handleRemoteMessage(connection.id, JSON.stringify(createPingCommand()));

    const status = service.disconnectClients();

    expect(status.connectedClientCount).toBe(0);
    expect(status.connectedClients).toEqual([]);
    expect(connection.close).toHaveBeenCalledTimes(1);
  });

  it('disconnects only the requested device', async () => {
    const service = createControlService();
    const matching = createConnection({ id: 'matching' });
    const other = createConnection({ id: 'other' });
    service.addRemoteConnection(matching);
    service.addRemoteConnection(other);
    await service.handleRemoteMessage(matching.id, JSON.stringify(createPingCommand()));
    await service.handleRemoteMessage(other.id, JSON.stringify(createPingCommand({ id: 'request-2', deviceId: 'android-2' })));

    service.disconnectDevice('android-1');

    expect(matching.close).toHaveBeenCalledTimes(1);
    expect(other.close).not.toHaveBeenCalled();
  });

  it('keeps pairing requests pending until accepted', async () => {
    const store = new MemoryPairingStore({ desktopId: 'desktop-1', pairedDevices: [] });
    const approvalManager = new PairingApprovalManager(store, () => now);
    const service = createControlService({
      store,
      pairingApprovalManager: approvalManager,
      authValidator: new CommandAuthValidator(store, () => now)
    });
    const connection = createConnection();
    service.addRemoteConnection(connection);

    await service.handleRemoteMessage(connection.id, JSON.stringify(createPairingRequest()));

    expect(service.getPendingPairingRequests()).toEqual([
      expect.objectContaining({
        requestId: 'approval-1',
        deviceName: 'Android smoke'
      })
    ]);

    await expect(service.respondToPairingRequest('approval-1', 'accept')).resolves.toEqual({ ok: true });
    const [response] = sentResponses(connection) as PairingCompleteResponse[];
    expect(response).toMatchObject({
      type: 'pairing.complete',
      payload: {
        desktopId: 'desktop-1',
        deviceId: 'android-smoke-1'
      }
    });
  });

  it('rejects pairing requests with a structured error', async () => {
    const store = new MemoryPairingStore({ desktopId: 'desktop-1', pairedDevices: [] });
    const approvalManager = new PairingApprovalManager(store, () => now);
    const service = createControlService({
      store,
      pairingApprovalManager: approvalManager,
      authValidator: new CommandAuthValidator(store, () => now)
    });
    const connection = createConnection();
    service.addRemoteConnection(connection);
    await service.handleRemoteMessage(connection.id, JSON.stringify(createPairingRequest()));

    await expect(service.respondToPairingRequest('approval-1', 'reject')).resolves.toEqual({ ok: true });

    expect(sentResponses(connection)).toEqual([
      expect.objectContaining({
        type: 'error',
        error: expect.objectContaining({ code: 'invalid_auth', message: 'pairing_rejected' })
      })
    ]);
    expect(connection.close).toHaveBeenCalledTimes(1);
  });

  it('includes Bluetooth status updates in control status', () => {
    const service = createControlService();

    service.setBluetoothStatus({
      ...DEFAULT_BLUETOOTH_STATUS,
      status: 'ready',
      reason: null,
      connectedClientCount: 0,
      lastError: null
    });

    expect(service.getStatus().bluetooth).toEqual({
      ...DEFAULT_BLUETOOTH_STATUS,
      status: 'ready',
      reason: null,
      connectedClientCount: 0,
      lastError: null
    });
    expect(service.getStatus().state).toBe('ready');
  });

  it('preserves live Bluetooth system status in control status', () => {
    const service = createControlService();

    service.setBluetoothStatus({
      ...DEFAULT_BLUETOOTH_STATUS,
      status: 'unavailable',
      reason: 'adapter_off',
      system: {
        adapterPresent: true,
        radioState: 'off',
        isLowEnergySupported: true,
        isPeripheralRoleSupported: true,
        lastCheckedAt: now,
        lastChangedAt: now
      }
    });

    expect(service.getStatus().bluetooth.system).toEqual({
      adapterPresent: true,
      radioState: 'off',
      isLowEnergySupported: true,
      isPeripheralRoleSupported: true,
      lastCheckedAt: now,
      lastChangedAt: now
    });
    expect(service.getStatus().state).toBe('error');
  });
});

function createControlService(
  overrides: Partial<ConstructorParameters<typeof ControlService>[0]> & { store?: MemoryPairingStore } = {}
): ControlService {
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
        },
        {
          deviceId: 'android-2',
          deviceName: 'Second Android device',
          token,
          pairedAt: now - 1_000,
          lastSeenAt: null
        }
      ]
    });

  return new ControlService({
    pairingManager: new PairingManager(store),
    authValidator: new CommandAuthValidator(store, () => now),
    ...overrides
  });
}

function createConnection(overrides: Partial<RemoteConnection> = {}): RemoteConnection {
  return {
    id: 'connection-1',
    kind: 'bluetooth',
    label: 'Bluetooth device',
    remoteAddress: null,
    send: vi.fn(),
    close: vi.fn(),
    ...overrides
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
