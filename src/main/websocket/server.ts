import { createServer, type IncomingMessage, type Server as HttpServer } from 'node:http';
import type { AddressInfo, ListenOptions } from 'node:net';
import { WebSocket, WebSocketServer } from 'ws';
import {
  createAckResponse,
  createErrorResponse,
  createPairingCompleteResponse,
  createPointerProfileResponse,
  parseProtocolRequest,
  type CommandRequest,
  type PairingApprovalRequest,
  type PointerMovementProfile
} from '../../shared/protocol';
import {
  DEFAULT_WS_PORT,
  type PcServerStatus,
  type PcServerListenerStatus,
  type PcServerStatusListener
} from '../../shared/server-status';
import type { CommandAuthValidator } from '../pairing/auth';
import type { PairingApprovalDecision, PendingPairingApprovalView } from '../../shared/pairing-approval';
import type { PairingApprovalManager } from '../pairing/pairing-approval-manager';
import type { PairingManager } from '../pairing/pairing-manager';
import { WebSocketClientRegistry } from './client-registry';
import { PendingPairingApprovalConnections } from './pending-approval-connections';
import { sendResponse, toProtocolCommandErrorCode } from './protocol-response';

export type PcWebSocketServerOptions = {
  port?: number;
  pairingManager: PairingManager;
  pairingApprovalManager?: PairingApprovalManager;
  authValidator: CommandAuthValidator;
  getPointerProfile?: () => PointerMovementProfile;
  onStatusChange?: PcServerStatusListener;
  onCommand?: (command: CommandRequest) => Promise<CommandHandlerResult> | CommandHandlerResult;
};

export type CommandHandlerResult =
  | { ok: true }
  | { ok: false; code: 'unsupported_command' | 'unsafe_payload' | 'adapter_failure'; message: string };

type ListenerFamily = 'IPv4' | 'IPv6';

type ListenerTarget = {
  family: ListenerFamily;
  host: string;
  ipv6Only?: boolean;
};

type PcWebSocketListener = {
  family: ListenerFamily;
  host: string;
  httpServer: HttpServer;
  wsServer: WebSocketServer;
  address: AddressInfo | null;
};

const LISTENER_TARGETS: ListenerTarget[] = [
  { family: 'IPv4', host: '0.0.0.0' },
  { family: 'IPv6', host: '::', ipv6Only: true }
];

export class PcWebSocketServer {
  private listeners: PcWebSocketListener[] = [];
  private readonly clientRegistry = new WebSocketClientRegistry();
  private readonly pendingApprovalConnections = new PendingPairingApprovalConnections();
  private status: PcServerStatus;

  constructor(private readonly options: PcWebSocketServerOptions) {
    this.status = {
      state: 'stopped',
      port: options.port ?? DEFAULT_WS_PORT,
      connectedClientCount: 0,
      connectedClients: [],
      lastSeenAt: null,
      lastError: null,
      listeners: []
    };
  }

  getStatus(): PcServerStatus {
    return {
      ...this.status,
      connectedClients: this.status.connectedClients.map((client) => ({ ...client })),
      listeners: this.status.listeners.map((listener) => ({ ...listener }))
    };
  }

  getAddresses(): PcServerListenerStatus[] {
    return this.getStatus().listeners;
  }

  async start(): Promise<PcServerStatus> {
    if (this.listeners.length > 0) return this.getStatus();

    this.setStatus({ state: 'starting', lastError: null, listeners: [] });

    const listenerStatuses: PcServerListenerStatus[] = [];
    const errors: string[] = [];
    let port = this.status.port;

    for (const target of LISTENER_TARGETS) {
      try {
        const listener = await this.startListener(target, port);
        this.listeners.push(listener);
        port = listener.address?.port ?? port;
        const status = toListenerStatus(listener);
        listenerStatuses.push(status);
        console.info(
          `Switchify WebSocket listener started. family=${status.family} address=${status.address} port=${status.port}`
        );
      } catch (error) {
        const message = error instanceof Error ? error.message : 'Unknown listener error.';
        const status = {
          family: target.family,
          address: target.host,
          port,
          state: 'error',
          error: message
        } satisfies PcServerListenerStatus;
        listenerStatuses.push(status);
        errors.push(`${target.family} ${target.host}:${port} failed: ${message}`);
        console.error(
          `Switchify WebSocket listener failed. family=${target.family} address=${target.host} port=${port} error=${message}`
        );
      }
    }

    if (this.listeners.length === 0) {
      const lastError = errors.join('; ') || 'No WebSocket listeners started.';
      this.setStatus({ state: 'error', lastError, listeners: listenerStatuses });
      throw new Error(lastError);
    }

    this.setStatus({
      state: 'listening',
      port,
      lastError: errors.length > 0 ? errors.join('; ') : null,
      listeners: listenerStatuses
    });

    return this.getStatus();
  }

  async stop(): Promise<PcServerStatus> {
    const listeners = this.listeners;
    if (listeners.length === 0) {
      this.setStatus({ state: 'stopped', connectedClientCount: 0, connectedClients: [], listeners: [] });
      return this.getStatus();
    }

    this.clientRegistry.closeAll();
    this.clientRegistry.clear();
    this.pendingApprovalConnections.clearAll();
    this.updateClientStatus();

    this.listeners = [];
    await Promise.all(listeners.map((listener) => closeListener(listener)));

    this.setStatus({ state: 'stopped', connectedClientCount: 0, connectedClients: [], listeners: [] });
    return this.getStatus();
  }

  disconnectClients(): PcServerStatus {
    this.clientRegistry.closeAll();
    this.clientRegistry.clear();
    this.updateClientStatus();
    return this.getStatus();
  }

  disconnectDevice(deviceId: string): PcServerStatus {
    this.clientRegistry.closeByDeviceId(deviceId);
    this.updateClientStatus();
    return this.getStatus();
  }

  getPendingPairingRequests(): PendingPairingApprovalView[] {
    this.expirePendingPairingRequests();
    return this.options.pairingApprovalManager?.listPendingRequestViews() ?? [];
  }

  async respondToPairingRequest(
    requestId: string,
    decision: PairingApprovalDecision
  ): Promise<{ ok: boolean; reason?: string }> {
    const manager = this.options.pairingApprovalManager;
    if (!manager) return { ok: false, reason: 'pairing_approval_unavailable' };

    if (decision === 'accept') {
      const result = await manager.accept(requestId);
      if (!result.ok) {
        this.pendingApprovalConnections.clear(requestId);
        return result;
      }

      const client = this.pendingApprovalConnections.get(requestId);
      if (client) {
        sendResponse(
          client,
          createPairingCompleteResponse(requestId, {
            desktopId: result.desktopId,
            deviceId: result.deviceId,
            token: result.token
          })
        );
        this.markClientSeen(client, result.deviceId);
      }
      this.pendingApprovalConnections.clear(requestId);
      return { ok: true };
    }

    const result = manager.reject(requestId);
    if (!result.ok) {
      this.pendingApprovalConnections.clear(requestId);
      return result;
    }

    const client = this.pendingApprovalConnections.get(requestId);
    if (client) {
      sendResponse(client, createErrorResponse(requestId, 'invalid_auth', 'pairing_rejected'));
      client.close();
    }
    this.pendingApprovalConnections.clear(requestId);
    return { ok: true };
  }

  private handleConnection(client: WebSocket, request: IncomingMessage): void {
    this.clientRegistry.add(client, request.socket.remoteAddress ?? null);
    this.updateClientStatus();

    client.on('message', (data) => {
      void this.handleMessage(client, data.toString());
    });
    client.on('close', () => {
      this.removePendingApprovalsForClient(client);
      this.clientRegistry.remove(client);
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
    if (message.type === 'pairing.request') {
      await this.handlePairingApprovalRequest(client, message);
      return;
    }

    const authResult = await this.options.authValidator.validate(message);
    if (!authResult.ok) {
      sendResponse(client, createErrorResponse(message.id, 'invalid_auth', authResult.reason));
      return;
    }

    if (authResult.command.type === 'pointer.profile') {
      const profile = this.getPointerProfile();
      if (!profile) {
        sendResponse(client, createErrorResponse(message.id, 'command_failed', 'Pointer profile is unavailable.'));
        return;
      }
      this.markClientSeen(client, authResult.command.deviceId);
      this.setStatus({ lastSeenAt: Date.now(), lastError: null });
      sendResponse(client, createPointerProfileResponse(message.id, profile));
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

  private async handlePairingApprovalRequest(client: WebSocket, message: PairingApprovalRequest): Promise<void> {
    const manager = this.options.pairingApprovalManager;
    if (!manager) {
      sendResponse(client, createErrorResponse(message.id, 'invalid_type', 'pairing_approval_unavailable'));
      return;
    }

    const desktopId = await this.options.pairingManager.getDesktopId();
    if (message.payload.desktopId !== desktopId) {
      sendResponse(client, createErrorResponse(message.id, 'invalid_auth', 'pairing_mismatch'));
      return;
    }

    const { request, replacedRequestId } = manager.createRequest({
      requestId: message.id,
      deviceId: message.payload.deviceId,
      deviceName: message.payload.deviceName,
      desktopId: message.payload.desktopId,
      requestNonce: message.payload.requestNonce,
      remoteAddress: this.clientRegistry.getRemoteAddress(client)
    });

    if (replacedRequestId) {
      const replacedClient = this.pendingApprovalConnections.get(replacedRequestId);
      if (replacedClient) {
        sendResponse(replacedClient, createErrorResponse(replacedRequestId, 'invalid_auth', 'pairing_request_expired'));
        replacedClient.close();
      }
      this.pendingApprovalConnections.clear(replacedRequestId);
    }

    this.pendingApprovalConnections.set(
      request.requestId,
      client,
      request.expiresAt,
      () => {
        this.expirePendingPairingRequests();
      }
    );
  }

  private updateClientStatus(): void {
    this.setStatus({
      connectedClientCount: this.clientRegistry.count(),
      connectedClients: this.clientRegistry.snapshot()
    });
  }

  private markClientSeen(client: WebSocket, deviceId: string): void {
    this.clientRegistry.markSeen(client, deviceId);
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

  private startListener(target: ListenerTarget, port: number): Promise<PcWebSocketListener> {
    return new Promise((resolve, reject) => {
      const httpServer = createServer();
      const wsServer = new WebSocketServer({ server: httpServer });
      const listenOptions = {
        host: target.host,
        port,
        ...(target.ipv6Only === undefined ? {} : { ipv6Only: target.ipv6Only })
      } satisfies ListenOptions;

      const cleanup = (): void => {
        httpServer.off('listening', onListening);
        httpServer.off('error', onError);
      };
      const onListening = (): void => {
        cleanup();
        wsServer.on('connection', (client, request) => this.handleConnection(client, request));
        const address = httpServer.address();
        resolve({
          family: target.family,
          host: target.host,
          httpServer,
          wsServer,
          address: typeof address === 'object' && address ? address : null
        });
      };
      const onError = (error: Error): void => {
        cleanup();
        wsServer.close();
        httpServer.close();
        reject(error);
      };

      httpServer.once('listening', onListening);
      httpServer.once('error', onError);
      httpServer.listen(listenOptions);
    });
  }

  private getPointerProfile(): PointerMovementProfile | null {
    try {
      return this.options.getPointerProfile?.() ?? null;
    } catch (error) {
      this.setStatus({
        lastError: error instanceof Error ? error.message : 'Pointer profile failed.'
      });
      return null;
    }
  }

  private expirePendingPairingRequests(): void {
    const expired = this.options.pairingApprovalManager?.expirePendingRequests() ?? [];
    for (const request of expired) {
      const client = this.pendingApprovalConnections.get(request.requestId);
      if (client) {
        sendResponse(client, createErrorResponse(request.requestId, 'invalid_auth', 'pairing_request_expired'));
        client.close();
      }
      this.pendingApprovalConnections.clear(request.requestId);
    }
  }

  private removePendingApprovalsForClient(client: WebSocket): void {
    for (const requestId of this.pendingApprovalConnections.clearForClient(client)) {
      this.options.pairingApprovalManager?.reject(requestId);
    }
  }
}

function toListenerStatus(listener: PcWebSocketListener): PcServerListenerStatus {
  return {
    family: listener.family,
    address: listener.address?.address ?? listener.host,
    port: listener.address?.port ?? 0,
    state: 'listening',
    error: null
  };
}

async function closeListener(listener: PcWebSocketListener): Promise<void> {
  await new Promise<void>((resolve) => {
    listener.wsServer.close(() => resolve());
  });
  await new Promise<void>((resolve, reject) => {
    listener.httpServer.close((error) => {
      if (error) reject(error);
      else resolve();
    });
  });
}
