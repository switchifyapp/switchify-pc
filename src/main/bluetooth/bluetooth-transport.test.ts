import { describe, expect, it, vi } from 'vitest';
import { PROTOCOL_VERSION, validateProtocolResponse, type PingCommand } from '../../shared/protocol';
import { createCommandAuthProof, CommandAuthValidator } from '../pairing/auth';
import { PairingManager } from '../pairing/pairing-manager';
import { MemoryPairingStore } from '../pairing/pairing-store';
import { PcWebSocketServer } from '../websocket/server';
import { WindowsBluetoothTransport } from './bluetooth-transport';
import type { BluetoothHelperClient } from './bluetooth-helper-client';
import type { BluetoothHelperEvent } from './helper-protocol';

const now = 1_724_000_000_000;
const token = 'shared-token';

describe('WindowsBluetoothTransport', () => {
  it('bridges Bluetooth frames into the shared remote session manager', async () => {
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
    const server = new PcWebSocketServer({
      port: 0,
      pairingManager: new PairingManager(store),
      authValidator: new CommandAuthValidator(store, () => now)
    });
    const fakeHelper = new FakeBluetoothHelper();
    const transport = new WindowsBluetoothTransport({
      server,
      getDesktopId: () => Promise.resolve('desktop-1'),
      displayName: 'Switchify PC',
      helperPath: 'fake-helper.exe',
      createHelper: (options) => {
        fakeHelper.onEvent = options.onEvent;
        return fakeHelper as unknown as BluetoothHelperClient;
      }
    });

    await transport.start();
    fakeHelper.emit({ type: 'ready' });
    fakeHelper.emit({ type: 'connected', connectionId: 'ble', label: 'Bluetooth device' });
    fakeHelper.emitFrame(JSON.stringify(createPingCommand()));
    await waitFor(() => fakeHelper.sentMessages().length === 1);

    const sent = fakeHelper.sentMessages();
    expect(sent).toHaveLength(1);
    expect(validateProtocolResponse(JSON.parse(sent[0]))).toMatchObject({ ok: true });
    expect(JSON.parse(sent[0])).toMatchObject({ type: 'ack', id: 'request-1', ok: true });
    expect(server.getStatus().connectedClients[0]).toMatchObject({
      deviceId: 'android-1',
      transport: 'bluetooth'
    });
  });
});

class FakeBluetoothHelper {
  onEvent: (event: BluetoothHelperEvent) => void = () => {};
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
