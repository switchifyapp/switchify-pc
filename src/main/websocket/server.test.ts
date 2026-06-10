import { WebSocket, type RawData } from 'ws';
import { afterEach, describe, expect, it } from 'vitest';
import {
  MAX_POINTER_DELTA,
  PROTOCOL_VERSION,
  validateProtocolResponse,
  type PairingCompleteResponse,
  type PingCommand,
  type PointerProfileCommand,
  type WindowControlCommand
} from '../../shared/protocol';
import { createCommandAuthProof, CommandAuthValidator } from '../pairing/auth';
import { PairingApprovalManager, PAIRING_APPROVAL_REQUEST_TTL_MS } from '../pairing/pairing-approval-manager';
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
    expect(started.listeners.some((listener) => listener.family === 'IPv4' && listener.state === 'listening')).toBe(
      true
    );

    const stopped = await server.stop();
    expect(stopped.state).toBe('stopped');
    expect(stopped.listeners).toEqual([]);
  });

  it('reports listener addresses and accepts IPv4 and available IPv6 connections', async () => {
    const server = createServer();
    activeServers.push(server);

    await server.start();
    const listeners = server.getAddresses();
    const ipv4Listener = listeners.find((listener) => listener.family === 'IPv4');
    const ipv6Listener = listeners.find((listener) => listener.family === 'IPv6');

    expect(ipv4Listener).toMatchObject({
      family: 'IPv4',
      address: '0.0.0.0',
      state: 'listening',
      error: null
    });

    const ipv4Client = await connect(server.getStatus().port, '127.0.0.1');
    ipv4Client.close();

    if (ipv6Listener?.state === 'listening') {
      expect(ipv6Listener).toMatchObject({
        family: 'IPv6',
        address: '::',
        error: null
      });
      const ipv6Client = await connect(server.getStatus().port, '::1');
      ipv6Client.close();
    }
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

  it('routes authenticated commands to the command handler before acking', async () => {
    const handled: PingCommand[] = [];
    const server = createServer({
      onCommand: (command) => {
        handled.push(command as PingCommand);
        return { ok: true };
      }
    });
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);
    const command = createPingCommand();

    const response = await sendAndReceive(client, command);

    expect(response).toMatchObject({ type: 'ack', id: command.id, ok: true });
    expect(handled).toEqual([command]);
    client.close();
  });

  it('routes authenticated window control commands and rejects invalid auth before execution', async () => {
    const handled: WindowControlCommand[] = [];
    const server = createServer({
      onCommand: (command) => {
        handled.push(command as WindowControlCommand);
        return { ok: true };
      }
    });
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);
    const command = createWindowControlCommand();

    const response = await sendAndReceive(client, command);

    expect(response).toMatchObject({ type: 'ack', id: command.id, ok: true });
    expect(handled).toEqual([command]);

    const invalidAuthResponse = await sendAndReceive(
      client,
      createWindowControlCommand({ id: 'window-control-bad-auth', auth: 'bad-proof' })
    );

    expect(invalidAuthResponse).toMatchObject({
      type: 'error',
      ok: false,
      error: { code: 'invalid_auth' }
    });
    expect(handled).toEqual([command]);
    client.close();
  });

  it('returns structured command errors when the handler fails', async () => {
    const server = createServer({
      onCommand: () => ({ ok: false, code: 'adapter_failure', message: 'Native input failed.' })
    });
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);

    const response = await sendAndReceive(client, createPingCommand());

    expect(response).toMatchObject({
      type: 'error',
      ok: false,
      error: { code: 'command_failed', message: 'Native input failed.' }
    });
    expect(server.getStatus().lastSeenAt).toBeNull();
    expect(server.getStatus().lastError).toBe('Native input failed.');
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

  it('returns an authenticated pointer profile without executing desktop input', async () => {
    const handled: unknown[] = [];
    const server = createServer({
      getPointerProfile: () => ({
        displayId: '0:0:1280:720:1.5',
        scaleFactor: 1.5,
        bounds: { x: 0, y: 0, width: 1280, height: 720 },
        maxDelta: MAX_POINTER_DELTA,
        recommendedDeltas: {
          small: 50,
          medium: 130,
          large: 252
        }
      }),
      onCommand: (command) => {
        handled.push(command);
        return { ok: true };
      }
    });
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);
    const command = createPointerProfileCommand();

    const response = await sendAndReceive(client, command);

    expect(response).toMatchObject({
      type: 'pointer.profile',
      id: command.id,
      ok: true,
      payload: {
        displayId: '0:0:1280:720:1.5',
        scaleFactor: 1.5,
        maxDelta: MAX_POINTER_DELTA,
        recommendedDeltas: {
          small: 50,
          medium: 130,
          large: 252
        }
      }
    });
    expect(handled).toEqual([]);
    client.close();
  });

  it('rejects pointer profile requests with invalid auth', async () => {
    const server = createServer({
      getPointerProfile: () => ({
        displayId: '0:0:1280:720:1.5',
        scaleFactor: 1.5,
        bounds: { x: 0, y: 0, width: 1280, height: 720 },
        maxDelta: MAX_POINTER_DELTA,
        recommendedDeltas: { small: 50, medium: 130, large: 252 }
      })
    });
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);

    const response = await sendAndReceive(client, createPointerProfileCommand({ auth: 'bad-proof' }));

    expect(response).toMatchObject({
      type: 'error',
      ok: false,
      error: { code: 'invalid_auth' }
    });
    client.close();
  });

  it('keeps pairing approval requests pending until accepted', async () => {
    const store = new MemoryPairingStore({ desktopId: 'desktop-1', pairedDevices: [] });
    const approvalManager = new PairingApprovalManager(store, () => now);
    const server = createServer({
      pairingManager: new PairingManager(store),
      pairingApprovalManager: approvalManager,
      authValidator: new CommandAuthValidator(store, () => now)
    });
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);
    const request = createPairingApprovalRequest();

    client.send(JSON.stringify(request));

    await expect(receiveWithin(client, 25)).resolves.toBeNull();
    expect(server.getPendingPairingRequests()).toMatchObject([
      {
        requestId: 'approval-1',
        deviceName: 'Android smoke',
        verificationCode: expect.stringMatching(/^\d{6}$/)
      }
    ]);

    await expect(server.respondToPairingRequest(request.id, 'accept')).resolves.toEqual({ ok: true });
    const response = await receive(client);

    expect(response).toMatchObject({
      type: 'pairing.complete',
      ok: true,
      payload: {
        desktopId: 'desktop-1',
        deviceId: 'android-smoke-1'
      }
    });
    expect(server.getPendingPairingRequests()).toHaveLength(0);
    client.close();
  });

  it('rejects pending pairing approval requests with a structured error', async () => {
    const store = new MemoryPairingStore({ desktopId: 'desktop-1', pairedDevices: [] });
    const approvalManager = new PairingApprovalManager(store, () => now);
    const server = createServer({
      pairingManager: new PairingManager(store),
      pairingApprovalManager: approvalManager,
      authValidator: new CommandAuthValidator(store, () => now)
    });
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);
    const request = createPairingApprovalRequest();
    client.send(JSON.stringify(request));
    await waitFor(() => server.getPendingPairingRequests().length === 1);

    await expect(server.respondToPairingRequest(request.id, 'reject')).resolves.toEqual({ ok: true });
    const response = await receive(client);

    expect(response).toMatchObject({
      type: 'error',
      ok: false,
      error: { code: 'invalid_auth', message: 'pairing_rejected' }
    });
    await waitFor(() => client.readyState === WebSocket.CLOSED);
  });

  it('expires pending pairing approval requests with a structured error', async () => {
    let currentTime = now;
    const store = new MemoryPairingStore({ desktopId: 'desktop-1', pairedDevices: [] });
    const approvalManager = new PairingApprovalManager(store, () => currentTime);
    const server = createServer({
      pairingManager: new PairingManager(store),
      pairingApprovalManager: approvalManager,
      authValidator: new CommandAuthValidator(store, () => currentTime)
    });
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);
    const request = createPairingApprovalRequest();
    client.send(JSON.stringify(request));
    await waitFor(() => server.getPendingPairingRequests().length === 1);
    currentTime += PAIRING_APPROVAL_REQUEST_TTL_MS + 1;

    server.getPendingPairingRequests();
    const response = await receive(client);

    expect(response).toMatchObject({
      type: 'error',
      ok: false,
      error: { code: 'invalid_auth', message: 'pairing_request_expired' }
    });
    await waitFor(() => client.readyState === WebSocket.CLOSED);
  });

  it('accepts authenticated ping after PC-approved pairing', async () => {
    const store = new MemoryPairingStore({ desktopId: 'desktop-1', pairedDevices: [] });
    const approvalManager = new PairingApprovalManager(store, () => now);
    const server = createServer({
      pairingManager: new PairingManager(store),
      pairingApprovalManager: approvalManager,
      authValidator: new CommandAuthValidator(store, () => now)
    });
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);
    const request = createPairingApprovalRequest();
    client.send(JSON.stringify(request));
    await waitFor(() => server.getPendingPairingRequests().length === 1);
    await server.respondToPairingRequest(request.id, 'accept');
    const pairingResponse = (await receive(client)) as PairingCompleteResponse;
    const ping = createPingCommand({
      id: 'approved-ping-1',
      deviceId: 'android-smoke-1',
      auth: createCommandAuthProof(
        {
          version: PROTOCOL_VERSION,
          id: 'approved-ping-1',
          deviceId: 'android-smoke-1',
          timestamp: now,
          type: 'connection.ping',
          payload: {},
          auth: ''
        },
        pairingResponse.payload.token
      )
    });

    const response = await sendAndReceive(client, ping);

    expect(response).toMatchObject({ type: 'ack', id: 'approved-ping-1', ok: true });
    client.close();
  });

  it('tracks connected clients and disconnects', async () => {
    const server = createServer();
    activeServers.push(server);
    await server.start();

    const client = await connect(server.getStatus().port);
    expect(server.getStatus().connectedClientCount).toBe(1);
    expect(server.getStatus().connectedClients).toHaveLength(1);

    await closeClient(client);
    await waitFor(() => server.getStatus().connectedClientCount === 0);
    expect(server.getStatus().connectedClientCount).toBe(0);
    expect(server.getStatus().connectedClients).toHaveLength(0);
  });

  it('records authenticated device ids in connected client status', async () => {
    const server = createServer();
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);

    await sendAndReceive(client, createPingCommand());

    expect(server.getStatus().connectedClients[0]).toMatchObject({
      deviceId: 'android-1'
    });
    client.close();
  });

  it('disconnects active clients without stopping the server', async () => {
    const server = createServer();
    activeServers.push(server);
    await server.start();
    const client = await connect(server.getStatus().port);

    const status = server.disconnectClients();

    expect(status.state).toBe('listening');
    expect(status.connectedClientCount).toBe(0);
    expect(status.connectedClients).toHaveLength(0);
    await waitFor(() => client.readyState === WebSocket.CLOSED);
  });
});

function createServer(overrides: Partial<ConstructorParameters<typeof PcWebSocketServer>[0]> = {}): PcWebSocketServer {
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
  return new PcWebSocketServer({
    port: 0,
    pairingManager: new PairingManager(store),
    authValidator: new CommandAuthValidator(store, () => now),
    ...overrides
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

function createPointerProfileCommand(overrides: Partial<PointerProfileCommand> = {}): PointerProfileCommand {
  const command = {
    version: PROTOCOL_VERSION,
    id: 'profile-1',
    deviceId: 'android-1',
    timestamp: now,
    type: 'pointer.profile',
    payload: {},
    auth: ''
  } satisfies PointerProfileCommand;
  const merged = { ...command, ...overrides } as PointerProfileCommand;
  return {
    ...merged,
    auth: overrides.auth ?? createCommandAuthProof(merged, token)
  };
}

function createWindowControlCommand(overrides: Partial<WindowControlCommand> = {}): WindowControlCommand {
  const command = {
    version: PROTOCOL_VERSION,
    id: 'window-control-1',
    deviceId: 'android-1',
    timestamp: now,
    type: 'window.control',
    payload: { action: 'switchNext' },
    auth: ''
  } satisfies WindowControlCommand;
  const merged = { ...command, ...overrides } as WindowControlCommand;
  return {
    ...merged,
    auth: overrides.auth ?? createCommandAuthProof(merged, token)
  };
}

function createPairingApprovalRequest() {
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

function connect(port: number, host = '127.0.0.1'): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const formattedHost = host.includes(':') ? `[${host}]` : host;
    const client = new WebSocket(`ws://${formattedHost}:${port}`);
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

function receive(client: WebSocket): Promise<unknown> {
  return new Promise((resolve, reject) => {
    client.once('message', (data) => {
      const parsed = JSON.parse(data.toString());
      expect(validateProtocolResponse(parsed)).toMatchObject({ ok: true });
      resolve(parsed);
    });
    client.once('error', reject);
  });
}

function receiveWithin(client: WebSocket, timeoutMs: number): Promise<unknown | null> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      client.off('message', onMessage);
      client.off('error', onError);
      resolve(null);
    }, timeoutMs);

    const onMessage = (data: RawData) => {
      clearTimeout(timer);
      client.off('error', onError);
      const parsed = JSON.parse(data.toString());
      expect(validateProtocolResponse(parsed)).toMatchObject({ ok: true });
      resolve(parsed);
    };
    const onError = (error: Error) => {
      clearTimeout(timer);
      client.off('message', onMessage);
      reject(error);
    };

    client.once('message', onMessage);
    client.once('error', onError);
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
