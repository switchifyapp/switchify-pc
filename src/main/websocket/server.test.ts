import { WebSocket } from 'ws';
import { afterEach, describe, expect, it } from 'vitest';
import { PROTOCOL_VERSION, validateProtocolResponse, type PingCommand } from '../../shared/protocol';
import { createCommandAuthProof, CommandAuthValidator } from '../pairing/auth';
import { PairingManager } from '../pairing/pairing-manager';
import { MemoryPairingStore } from '../pairing/pairing-store';
import { PcWebSocketServer } from './server';

const now = 1_724_000_000_000;
const token = 'shared-token';
const activeServers: PcWebSocketServer[] = [];

afterEach(async () => {
  await Promise.all(activeServers.splice(0).map((server) => server.stop()));
});

describe('PcWebSocketServer', () => {
  it('starts and stops cleanly', async () => {
    const server = createServer();
    activeServers.push(server);

    const started = await server.start();
    expect(started.state).toBe('listening');
    expect(started.port).toBeGreaterThan(0);

    const stopped = await server.stop();
    expect(stopped.state).toBe('stopped');
  });

  it('acks authenticated ping commands', async () => {
    const server = createServer();
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);
    const command = createPingCommand();

    const response = await sendAndReceive(client, command);

    expect(response).toMatchObject({ type: 'ack', id: command.id, ok: true });
    client.close();
  });

  it('returns structured errors for invalid auth without crashing', async () => {
    const server = createServer();
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);

    const response = await sendAndReceive(client, createPingCommand({ auth: 'bad-proof' }));

    expect(response).toMatchObject({
      type: 'error',
      ok: false,
      error: { code: 'invalid_auth' }
    });
    expect(server.getStatus().state).toBe('listening');
    client.close();
  });

  it('accepts pairing messages and returns a token response', async () => {
    const store = new MemoryPairingStore();
    const pairingManager = new PairingManager(store, () => now);
    const pairingSession = await pairingManager.createPairingSession();
    const server = new PcWebSocketServer({
      port: 0,
      pairingManager,
      authValidator: new CommandAuthValidator(store, () => now)
    });
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);

    await expect(
      sendAndReceive(client, {
        version: PROTOCOL_VERSION,
        id: 'pairing-start-1',
        type: 'pairing.start',
        payload: {
          deviceId: 'android-1',
          deviceName: 'Android phone',
          pairingCode: pairingSession.pairingCode
        }
      })
    ).resolves.toMatchObject({ type: 'ack', ok: true });

    const response = await sendAndReceive(client, {
      version: PROTOCOL_VERSION,
      id: 'pairing-complete-1',
      type: 'pairing.complete',
      payload: {
        deviceId: 'android-1',
        desktopId: pairingSession.desktopId,
        pairingNonce: pairingSession.pairingNonce
      }
    });

    expect(response).toMatchObject({
      type: 'pairing.complete',
      ok: true,
      payload: {
        desktopId: pairingSession.desktopId,
        deviceId: 'android-1'
      }
    });
    expect((await store.load()).pairedDevices[0]).toMatchObject({
      deviceId: 'android-1',
      deviceName: 'Android phone'
    });
    client.close();
  });

  it('tracks connected clients and disconnects', async () => {
    const server = createServer();
    activeServers.push(server);
    await server.start();

    const client = await connect(server.getStatus().port);
    expect(server.getStatus().connectedClientCount).toBe(1);

    await closeClient(client);
    await waitFor(() => server.getStatus().connectedClientCount === 0);
    expect(server.getStatus().connectedClientCount).toBe(0);
  });
});

function createServer(): PcWebSocketServer {
  const store = new MemoryPairingStore({
    desktopId: 'desktop-1',
    pairedDevices: [
      {
        deviceId: 'android-1',
        deviceName: 'Android phone',
        token,
        pairedAt: now - 1_000,
        lastSeenAt: null
      }
    ]
  });
  return new PcWebSocketServer({
    port: 0,
    pairingManager: new PairingManager(store, () => now),
    authValidator: new CommandAuthValidator(store, () => now)
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

function connect(port: number): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const client = new WebSocket(`ws://127.0.0.1:${port}`);
    client.once('open', () => resolve(client));
    client.once('error', reject);
  });
}

function sendAndReceive(client: WebSocket, message: unknown): Promise<unknown> {
  return new Promise((resolve, reject) => {
    client.once('message', (data) => {
      const parsed = JSON.parse(data.toString());
      expect(validateProtocolResponse(parsed)).toMatchObject({ ok: true });
      resolve(parsed);
    });
    client.once('error', reject);
    client.send(JSON.stringify(message));
  });
}

function closeClient(client: WebSocket): Promise<void> {
  return new Promise((resolve) => {
    client.once('close', () => resolve());
    client.close();
  });
}

async function waitFor(predicate: () => boolean): Promise<void> {
  const deadline = Date.now() + 1_000;
  while (Date.now() < deadline) {
    if (predicate()) return;
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  throw new Error('Timed out waiting for condition.');
}
