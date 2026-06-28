import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PROTOCOL_VERSION, validateProtocolResponse, type PingCommand } from '../../shared/protocol';
import { createCommandAuthProof, CommandAuthValidator } from '../pairing/auth';
import { PairingManager } from '../pairing/pairing-manager';
import { MemoryPairingStore } from '../pairing/pairing-store';
import { ControlService } from '../control/control-service';
import { WindowsBluetoothTransport } from './bluetooth-transport';
import type { BluetoothHelperClient } from './bluetooth-helper-client';
import type { BluetoothHelperEvent } from './helper-protocol';

const now = 1_724_000_000_000;
const token = 'shared-token';

beforeEach(() => {
  vi.spyOn(Date, 'now').mockReturnValue(now);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('WindowsBluetoothTransport', () => {
  it('bridges Bluetooth frames into the shared remote session manager', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({ type: 'ready' });
    fakeHelper.emit({ type: 'connected', connectionId: 'ble', label: 'Bluetooth device' });
    fakeHelper.emitFrame(JSON.stringify(createPingCommand()));
    await waitFor(() => fakeHelper.sentMessages().length === 1);

    const sent = fakeHelper.sentMessages();
    expect(sent).toHaveLength(1);
    expect(validateProtocolResponse(JSON.parse(sent[0]))).toMatchObject({ ok: true });
    expect(JSON.parse(sent[0])).toMatchObject({ type: 'ack', id: 'request-1', ok: true });
    expect(controlService.getStatus().connectedClients[0]).toMatchObject({
      deviceId: 'android-1',
      transport: 'bluetooth'
    });
  });

  it('records diagnostic events in Bluetooth status', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({ type: 'diagnostic', event: 'subscribed' });
    fakeHelper.emit({ type: 'diagnostic', event: 'unsubscribed' });

    expect(controlService.getStatus().bluetooth).toMatchObject({
      lastEvent: 'unsubscribed',
      lastEventAt: now,
      recentEvents: [
        { event: 'subscribed', at: now },
        { event: 'unsubscribed', at: now }
      ]
    });
  });

  it('keeps only the most recent Bluetooth diagnostic events', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({ type: 'diagnostic', event: 'advertising_started' });
    fakeHelper.emit({ type: 'diagnostic', event: 'subscribed' });
    fakeHelper.emit({ type: 'diagnostic', event: 'unsubscribed' });
    fakeHelper.emit({ type: 'diagnostic', event: 'unsubscribe_grace_started' });
    fakeHelper.emit({ type: 'diagnostic', event: 'unsubscribe_grace_cancelled' });
    fakeHelper.emit({ type: 'diagnostic', event: 'write_received' });

    expect(controlService.getStatus().bluetooth.recentEvents.map((event) => event.event)).toEqual([
      'subscribed',
      'unsubscribed',
      'unsubscribe_grace_started',
      'unsubscribe_grace_cancelled',
      'write_received'
    ]);
  });

  it('records disconnect reasons in Bluetooth status', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({ type: 'connected', connectionId: 'ble', label: 'Bluetooth device' });
    fakeHelper.emit({ type: 'disconnected', connectionId: 'ble', reason: 'notification_unsubscribed' });

    expect(controlService.getStatus().bluetooth).toMatchObject({
      status: 'ready',
      connectedClientCount: 0,
      lastDisconnectReason: 'notification_unsubscribed',
      lastDisconnectAt: now
    });
  });

  it('overrides subscription timeout when the client announced an intentional disconnect', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({ type: 'connected', connectionId: 'ble', label: 'Bluetooth device' });
    transport.markClientRequestedDisconnect('ble');
    fakeHelper.emit({ type: 'disconnected', connectionId: 'ble', reason: 'notification_unsubscribed' });

    expect(controlService.getStatus().bluetooth).toMatchObject({
      status: 'ready',
      connectedClientCount: 0,
      lastDisconnectReason: 'client_requested',
      lastDisconnectAt: now
    });
  });

  it('does not let client disconnect intent override helper failures', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({ type: 'connected', connectionId: 'ble', label: 'Bluetooth device' });
    transport.markClientRequestedDisconnect('ble');
    fakeHelper.fail('helper crashed');

    expect(controlService.getStatus().bluetooth).toMatchObject({
      status: 'error',
      connectedClientCount: 0,
      lastDisconnectReason: 'helper_error',
      lastDisconnectAt: now
    });
  });

  it('clears client disconnect intent when Bluetooth traffic resumes before timeout', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({ type: 'connected', connectionId: 'ble', label: 'Bluetooth device' });
    transport.markClientRequestedDisconnect('ble');
    fakeHelper.emitFrame(JSON.stringify(createPingCommand()));
    await waitFor(() => fakeHelper.sentMessages().length === 1);
    fakeHelper.emit({ type: 'disconnected', connectionId: 'ble', reason: 'notification_unsubscribed' });

    expect(controlService.getStatus().bluetooth).toMatchObject({
      status: 'ready',
      connectedClientCount: 0,
      lastDisconnectReason: 'notification_unsubscribed',
      lastDisconnectAt: now
    });
  });

  it('clears active connections when the helper fails', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({ type: 'connected', connectionId: 'ble', label: 'Bluetooth device' });
    fakeHelper.fail('helper crashed');

    expect(controlService.getStatus().bluetooth).toMatchObject({
      status: 'error',
      connectedClientCount: 0,
      lastDisconnectReason: 'helper_error',
      lastDisconnectAt: now,
      lastError: 'helper crashed'
    });
  });

  it('includes live system Bluetooth status with checked and changed timestamps', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({
      type: 'systemStatus',
      adapterPresent: true,
      radioState: 'on',
      isLowEnergySupported: true,
      isPeripheralRoleSupported: true
    });

    expect(controlService.getStatus().bluetooth.system).toEqual({
      adapterPresent: true,
      radioState: 'on',
      isLowEnergySupported: true,
      isPeripheralRoleSupported: true,
      lastCheckedAt: now,
      lastChangedAt: now
    });
  });

  it('updates checked timestamp without changing changed timestamp for identical system status', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({
      type: 'systemStatus',
      adapterPresent: true,
      radioState: 'on',
      isLowEnergySupported: true,
      isPeripheralRoleSupported: true
    });
    vi.mocked(Date.now).mockReturnValue(now + 5_000);
    fakeHelper.emit({
      type: 'systemStatus',
      adapterPresent: true,
      radioState: 'on',
      isLowEnergySupported: true,
      isPeripheralRoleSupported: true
    });

    expect(controlService.getStatus().bluetooth.system).toMatchObject({
      lastCheckedAt: now + 5_000,
      lastChangedAt: now
    });
  });

  it('updates changed timestamp when system radio state changes', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({
      type: 'systemStatus',
      adapterPresent: true,
      radioState: 'on',
      isLowEnergySupported: true,
      isPeripheralRoleSupported: true
    });
    vi.mocked(Date.now).mockReturnValue(now + 5_000);
    fakeHelper.emit({
      type: 'systemStatus',
      adapterPresent: true,
      radioState: 'off',
      isLowEnergySupported: true,
      isPeripheralRoleSupported: true
    });

    expect(controlService.getStatus().bluetooth.system).toMatchObject({
      radioState: 'off',
      lastCheckedAt: now + 5_000,
      lastChangedAt: now + 5_000
    });
  });

  it('removes active connections when the system radio turns off', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({ type: 'connected', connectionId: 'ble', label: 'Bluetooth device' });
    fakeHelper.emit({
      type: 'systemStatus',
      adapterPresent: true,
      radioState: 'off',
      isLowEnergySupported: true,
      isPeripheralRoleSupported: true
    });

    expect(controlService.getStatus().bluetooth).toMatchObject({
      status: 'unavailable',
      reason: 'adapter_off',
      connectedClientCount: 0,
      lastDisconnectReason: 'adapter_off',
      lastDisconnectAt: now
    });
    expect(controlService.getStatus().connectedClients).toEqual([]);
  });

  it('moves to starting when the system radio comes back on after adapter off', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({
      type: 'systemStatus',
      adapterPresent: true,
      radioState: 'off',
      isLowEnergySupported: true,
      isPeripheralRoleSupported: true
    });
    fakeHelper.emit({
      type: 'systemStatus',
      adapterPresent: true,
      radioState: 'on',
      isLowEnergySupported: true,
      isPeripheralRoleSupported: true
    });

    expect(controlService.getStatus().bluetooth).toMatchObject({
      status: 'starting',
      reason: null
    });

    fakeHelper.emit({ type: 'ready' });

    expect(controlService.getStatus().bluetooth).toMatchObject({
      status: 'ready',
      reason: null
    });
  });

  it('reports unsupported when the system adapter is missing', async () => {
    const { controlService, fakeHelper, transport } = createTransport();

    await transport.start();
    fakeHelper.emit({
      type: 'systemStatus',
      adapterPresent: false,
      radioState: 'unknown',
      isLowEnergySupported: null,
      isPeripheralRoleSupported: null
    });

    expect(controlService.getStatus().bluetooth).toMatchObject({
      status: 'unavailable',
      reason: 'unsupported',
      connectedClientCount: 0,
      system: {
        adapterPresent: false,
        radioState: 'unknown',
        isLowEnergySupported: null,
        isPeripheralRoleSupported: null,
        lastCheckedAt: now,
        lastChangedAt: now
      }
    });
  });

  it('notifies status listeners when system Bluetooth status changes', async () => {
    const statusChanges: unknown[] = [];
    const { fakeHelper, transport } = createTransport({
      onStatusChange: (status) => statusChanges.push(status)
    });

    await transport.start();
    fakeHelper.emit({
      type: 'systemStatus',
      adapterPresent: true,
      radioState: 'on',
      isLowEnergySupported: true,
      isPeripheralRoleSupported: true
    });

    expect(statusChanges).toEqual([
      expect.objectContaining({
        status: 'starting'
      }),
      expect.objectContaining({
        system: expect.objectContaining({ radioState: 'on' })
      })
    ]);
  });
});

function createTransport(options: Pick<ConstructorParameters<typeof WindowsBluetoothTransport>[0], 'onStatusChange'> = {}): {
  controlService: ControlService;
  fakeHelper: FakeBluetoothHelper;
  transport: WindowsBluetoothTransport;
} {
  const store = new MemoryPairingStore({
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
  const controlService = new ControlService({
    pairingManager: new PairingManager(store),
    authValidator: new CommandAuthValidator(store, () => now)
  });
  const fakeHelper = new FakeBluetoothHelper();
  const transport = new WindowsBluetoothTransport({
    controlService,
    getDesktopId: () => Promise.resolve('desktop-1'),
    displayName: 'Switchify PC',
    helperPath: 'fake-helper.exe',
    ...options,
    createHelper: (options) => {
      fakeHelper.onEvent = options.onEvent;
      fakeHelper.onFailure = options.onFailure ?? (() => {});
      return fakeHelper as unknown as BluetoothHelperClient;
    }
  });
  return { controlService, fakeHelper, transport };
}

class FakeBluetoothHelper {
  onEvent: (event: BluetoothHelperEvent) => void = () => {};
  onFailure: (message: string) => void = () => {};
  private readonly sentFrames: string[] = [];

  start(): boolean {
    return true;
  }

  stop(): void {}

  destroy(): void {}

  disconnect(): void {}

  send(_connectionId: string, frame: { payloadBase64: string }): void {
    this.sentFrames.push(Buffer.from(frame.payloadBase64, 'base64').toString('utf8'));
  }

  emit(event: BluetoothHelperEvent): void {
    this.onEvent(event);
  }

  fail(message: string): void {
    this.onFailure(message);
  }

  emitFrame(message: string): void {
    this.emit({
      type: 'message',
      connectionId: 'ble',
      frame: {
        version: 1,
        messageId: 'inbound-1',
        sequence: 0,
        isFinal: true,
        totalBytes: Buffer.byteLength(message, 'utf8'),
        payloadBase64: Buffer.from(message, 'utf8').toString('base64')
      }
    });
  }

  sentMessages(): string[] {
    return this.sentFrames;
  }
}

async function waitFor(predicate: () => boolean): Promise<void> {
  const deadline = Date.now() + 1_000;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw new Error('Timed out waiting for condition.');
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
