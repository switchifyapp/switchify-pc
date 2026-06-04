import type { IncomingMessage } from 'node:http';
import { randomUUID } from 'node:crypto';
import { WebSocket, WebSocketServer } from 'ws';
import {
  createAckResponse,
  createErrorResponse,
  createPairingCompleteResponse,
  parseProtocolRequest,
  type CommandRequest,
  type PairingCompleteRequest,
  type PairingStartRequest,
  type ProtocolErrorCode,
  type ProtocolResponse
} from '../../shared/protocol';
import {
  DEFAULT_WS_PORT,
  type PcConnectedClient,
  type PcServerStatus,
  type PcServerStatusListener
} from '../../shared/server-status';
import type { CommandAuthValidator } from '../pairing/auth';
import type { PairingManager } from '../pairing/pairing-manager';

export type PcWebSocketServerOptions = {
  port?: number;
  pairingManager: PairingManager;
  authValidator: CommandAuthValidator;
  onStatusChange?: PcServerStatusListener;
  onCommand?: (command: CommandRequest) => Promise<CommandHandlerResult> | CommandHandlerResult;
};

export type CommandHandlerResult =
  | { ok: true }
  | { ok: false; code: 'unsupported_command' | 'unsafe_payload' | 'adapter_failure'; message: string };

type CommandHandlerFailureCode = Extract<CommandHandlerResult, { ok: false }>['code'];

export class PcWebSocketServer {
  private server: WebSocketServer | null = null;
  private readonly clients = new Map<WebSocket, PcConnectedClient>();
  private readonly pendingPairingDeviceNames = new Map<string, string>();
  private status: PcServerStatus;

  constructor(private readonly options: PcWebSocketServerOptions) {
    this.status = {
      state: 'stopped',
      port: options.port ?? DEFAULT_WS_PORT,
      connectedClientCount: 0,
      connectedClients: [],
      lastSeenAt: null,
      lastError: null
    };
  }

  getStatus(): PcServerStatus {
    return {
      ...this.status,
      connectedClients: this.status.connectedClients.map((client) => ({ ...client }))
    };
  }

  async start(): Promise<PcServerStatus> {
    if (this.server) return this.getStatus();

    this.setStatus({ state: 'starting', lastError: null });

    await new Promise<void>((resolve, reject) => {
      const server = new WebSocketServer({ port: this.status.port });
      this.server = server;

      server.once('listening', () => {
        const address = server.address();
        const port = typeof address === 'object' && address ? address.port : this.status.port;
        this.setStatus({ state: 'listening', port });
        resolve();
      });
      server.once('error', (error) => {
        this.setStatus({ state: 'error', lastError: error.message });
        reject(error);
      });
      server.on('connection', (client, request) => this.handleConnection(client, request));
    });

    return this.getStatus();
  }

  async stop(): Promise<PcServerStatus> {
    const server = this.server;
    if (!server) {
      this.setStatus({ state: 'stopped', connectedClientCount: 0, connectedClients: [] });
      return this.getStatus();
    }

    for (const client of this.clients.keys()) {
      client.close();
    }
    this.clients.clear();
    this.updateClientStatus();

    await new Promise<void>((resolve, reject) => {
      server.close((error) => {
        if (error) reject(error);
        else resolve();
      });
    });

    this.server = null;
    this.setStatus({ state: 'stopped', connectedClientCount: 0, connectedClients: [] });
    return this.getStatus();
  }

  disconnectClients(): PcServerStatus {
    for (const client of this.clients.keys()) {
      client.close();
    }
    this.clients.clear();
    this.updateClientStatus();
    return this.getStatus();
  }

  private handleConnection(client: WebSocket, request: IncomingMessage): void {
    this.clients.set(client, {
      id: randomUUID(),
      deviceId: null,
      remoteAddress: request.socket.remoteAddress ?? null,
      connectedAt: Date.now(),
      lastSeenAt: null
    });
    this.updateClientStatus();

    client.on('message', (data) => {
      void this.handleMessage(client, data.toString());
    });
    client.on('close', () => {
      this.clients.delete(client);
      this.updateClientStatus();
    });
    client.on('error', (error) => {
      this.setStatus({ lastError: error.message });
    });
  }

  private async handleMessage(client: WebSocket, rawMessage: string): Promise<void> {
    const parsed = parseProtocolRequest(rawMessage);
    if (!parsed.ok) {
      sendResponse(client, createErrorResponse(null, parsed.error, parsed.message));
      return;
    }

    const message = parsed.value;
    if (message.type === 'pairing.start') {
      await this.handlePairingStart(client, message);
      return;
    }
    if (message.type === 'pairing.complete') {
      await this.handlePairingComplete(client, message);
      return;
    }

    const authResult = await this.options.authValidator.validate(message);
    if (!authResult.ok) {
      sendResponse(client, createErrorResponse(message.id, 'invalid_auth', authResult.reason));
      return;
    }

    const commandResult = await this.executeCommand(authResult.command);
    if (!commandResult.ok) {
      this.setStatus({ lastError: commandResult.message });
      sendResponse(
        client,
        createErrorResponse(message.id, toProtocolCommandErrorCode(commandResult.code), commandResult.message)
      );
      return;
    }

    this.markClientSeen(client, authResult.command.deviceId);
    this.setStatus({ lastSeenAt: Date.now(), lastError: null });
    sendResponse(client, createAckResponse(message.id));
  }

  private async handlePairingStart(client: WebSocket, message: PairingStartRequest): Promise<void> {
    const session = this.options.pairingManager.getActivePairingSession();
    if (!session || session.pairingCode !== message.payload.pairingCode) {
      sendResponse(client, createErrorResponse(message.id, 'invalid_auth', 'pairing_mismatch'));
      return;
    }

    this.pendingPairingDeviceNames.set(message.payload.deviceId, message.payload.deviceName);
    sendResponse(client, createAckResponse(message.id));
  }

  private async handlePairingComplete(client: WebSocket, message: PairingCompleteRequest): Promise<void> {
    const session = this.options.pairingManager.getActivePairingSession();
    if (!session || session.desktopId !== message.payload.desktopId) {
      sendResponse(client, createErrorResponse(message.id, 'invalid_auth', 'pairing_mismatch'));
      return;
    }

    const result = await this.options.pairingManager.completePairing({
      deviceId: message.payload.deviceId,
      deviceName: this.pendingPairingDeviceNames.get(message.payload.deviceId) ?? message.payload.deviceId,
      pairingCode: session.pairingCode,
      pairingNonce: message.payload.pairingNonce
    });
    if (!result.ok) {
      sendResponse(client, createErrorResponse(message.id, 'invalid_auth', result.reason));
      return;
    }

    sendResponse(
      client,
      createPairingCompleteResponse(message.id, {
        desktopId: result.desktopId,
        deviceId: result.deviceId,
        token: result.token
      })
    );
    this.pendingPairingDeviceNames.delete(message.payload.deviceId);
  }

  private updateClientStatus(): void {
    this.setStatus({
      connectedClientCount: this.clients.size,
      connectedClients: [...this.clients.values()].map((client) => ({ ...client }))
    });
  }

  private markClientSeen(client: WebSocket, deviceId: string): void {
    const existing = this.clients.get(client);
    if (!existing) return;

    this.clients.set(client, {
      ...existing,
      deviceId,
      lastSeenAt: Date.now()
    });
    this.updateClientStatus();
  }

  private async executeCommand(command: CommandRequest): Promise<CommandHandlerResult> {
    try {
      return (await this.options.onCommand?.(command)) ?? { ok: true };
    } catch (error) {
      return {
        ok: false,
        code: 'adapter_failure',
        message: error instanceof Error ? error.message : 'Command execution failed.'
      };
    }
  }

  private setStatus(update: Partial<PcServerStatus>): void {
    this.status = { ...this.status, ...update };
    this.options.onStatusChange?.(this.getStatus());
  }
}

function toProtocolCommandErrorCode(code: CommandHandlerFailureCode): ProtocolErrorCode {
  if (code === 'unsafe_payload') return 'invalid_payload';
  if (code === 'unsupported_command') return 'invalid_type';
  return 'command_failed';
}

function sendResponse(client: WebSocket, response: ProtocolResponse): void {
  if (client.readyState === WebSocket.OPEN) {
    client.send(JSON.stringify(response));
  }
}
