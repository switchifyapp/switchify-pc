import type { IncomingMessage } from 'node:http';
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

export class PcWebSocketServer {
  private server: WebSocketServer | null = null;
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

    this.clientRegistry.closeAll();
    this.clientRegistry.clear();
    this.pendingApprovalConnections.clearAll();
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
    this.clientRegistry.closeAll();
    this.clientRegistry.clear();
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
