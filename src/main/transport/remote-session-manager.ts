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
import type { PcConnectedClient } from '../../shared/server-status';
import type { PairingApprovalDecision, PendingPairingApprovalView } from '../../shared/pairing-approval';
import type { CommandAuthValidator } from '../pairing/auth';
import type { PairingApprovalManager } from '../pairing/pairing-approval-manager';
import type { PairingManager } from '../pairing/pairing-manager';
import { PendingPairingApprovalConnections } from './pending-approval-connections';
import type { RemoteConnection } from './remote-connection';
import { RemoteClientRegistry } from './remote-client-registry';
import { sendResponse, toProtocolCommandErrorCode } from './protocol-response';

export type CommandHandlerResult =
  | { ok: true }
  | { ok: false; code: 'unsupported_command' | 'unsafe_payload' | 'adapter_failure'; message: string };

export type RemoteSessionManagerOptions = {
  pairingManager: PairingManager;
  pairingApprovalManager?: PairingApprovalManager;
  authValidator: CommandAuthValidator;
  getPointerProfile?: () => PointerMovementProfile;
  onClientStatusChange?: () => void;
  onLastSeen?: (seenAt: number) => void;
  onError?: (message: string) => void;
  onClientDisconnecting?: (connectionId: string, deviceId: string) => void;
  onCommand?: (command: CommandRequest) => Promise<CommandHandlerResult> | CommandHandlerResult;
};

export class RemoteSessionManager {
  private readonly clientRegistry = new RemoteClientRegistry();
  private readonly pendingApprovalConnections = new PendingPairingApprovalConnections();

  constructor(private readonly options: RemoteSessionManagerOptions) {}

  addConnection(connection: RemoteConnection): PcConnectedClient {
    const client = this.clientRegistry.add(connection);
    this.notifyClientStatusChanged();
    return client;
  }

  removeConnection(connectionId: string): void {
    this.removePendingApprovalsForConnection(connectionId);
    this.clientRegistry.remove(connectionId);
    this.notifyClientStatusChanged();
  }

  async closeAllConnections(): Promise<void> {
    const closePromise = this.clientRegistry.closeAll();
    this.clientRegistry.clear();
    this.pendingApprovalConnections.clearAll();
    this.notifyClientStatusChanged();
    await closePromise;
  }

  clearConnections(): void {
    this.clientRegistry.clear();
    this.pendingApprovalConnections.clearAll();
    this.notifyClientStatusChanged();
  }

  async disconnectDevice(deviceId: string): Promise<number> {
    const closePromise = this.clientRegistry.closeByDeviceId(deviceId);
    this.notifyClientStatusChanged();
    return await closePromise;
  }

  getAuthenticatedClientCount(): number {
    return this.clientRegistry.authenticatedCount();
  }

  getAuthenticatedClients(): PcConnectedClient[] {
    return this.clientRegistry.authenticatedSnapshot();
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

      const connection = this.getPendingConnection(requestId);
      if (connection) {
        await sendResponse(
          connection,
          createPairingCompleteResponse(requestId, {
            desktopId: result.desktopId,
            deviceId: result.deviceId,
            token: result.token
          })
        );
        this.markConnectionSeen(connection.id, result.deviceId);
      }
      this.pendingApprovalConnections.clear(requestId);
      return { ok: true };
    }

    const result = manager.reject(requestId);
    if (!result.ok) {
      this.pendingApprovalConnections.clear(requestId);
      return result;
    }

    const connection = this.getPendingConnection(requestId);
    if (connection) {
      await sendResponse(connection, createErrorResponse(requestId, 'invalid_auth', 'pairing_rejected'));
      await connection.close();
    }
    this.pendingApprovalConnections.clear(requestId);
    return { ok: true };
  }

  async handleMessage(connectionId: string, rawMessage: string): Promise<void> {
    const connection = this.clientRegistry.getConnection(connectionId);
    if (!connection) return;

    const parsed = parseProtocolRequest(rawMessage);
    if (!parsed.ok) {
      await sendResponse(connection, createErrorResponse(null, parsed.error, parsed.message));
      return;
    }

    const message = parsed.value;
    if (message.type === 'pairing.request') {
      await this.handlePairingApprovalRequest(connection, message);
      return;
    }

    const authResult = await this.options.authValidator.validate(message);
    if (!authResult.ok) {
      await sendResponse(connection, createErrorResponse(message.id, 'invalid_auth', authResult.reason));
      return;
    }

    if (authResult.command.type === 'pointer.profile') {
      const profile = this.getPointerProfile();
      if (!profile) {
        await sendResponse(connection, createErrorResponse(message.id, 'command_failed', 'Pointer profile is unavailable.'));
        return;
      }
      this.markConnectionSeen(connection.id, authResult.command.deviceId);
      this.markLastSeen(Date.now());
      await sendResponse(connection, createPointerProfileResponse(message.id, profile));
      return;
    }

    if (authResult.command.type === 'connection.disconnecting') {
      this.markConnectionSeen(connection.id, authResult.command.deviceId);
      this.markLastSeen(Date.now());
      this.options.onClientDisconnecting?.(connection.id, authResult.command.deviceId);
      await sendResponse(connection, createAckResponse(message.id));
      return;
    }

    const commandResult = await this.executeCommand(authResult.command);
    if (!commandResult.ok) {
      this.options.onError?.(commandResult.message);
      await sendResponse(
        connection,
        createErrorResponse(message.id, toProtocolCommandErrorCode(commandResult.code), commandResult.message)
      );
      return;
    }

    this.markConnectionSeen(connection.id, authResult.command.deviceId);
    this.markLastSeen(Date.now());
    await sendResponse(connection, createAckResponse(message.id));
  }

  private async handlePairingApprovalRequest(
    connection: RemoteConnection,
    message: PairingApprovalRequest
  ): Promise<void> {
    const manager = this.options.pairingApprovalManager;
    if (!manager) {
      await sendResponse(connection, createErrorResponse(message.id, 'invalid_type', 'pairing_approval_unavailable'));
      return;
    }

    const desktopId = await this.options.pairingManager.getDesktopId();
    if (message.payload.desktopId !== desktopId) {
      await sendResponse(connection, createErrorResponse(message.id, 'invalid_auth', 'pairing_mismatch'));
      return;
    }

    const { request, replacedRequestId } = manager.createRequest({
      requestId: message.id,
      deviceId: message.payload.deviceId,
      deviceName: message.payload.deviceName,
      desktopId: message.payload.desktopId,
      requestNonce: message.payload.requestNonce,
      remoteAddress: this.clientRegistry.getRemoteAddress(connection.id)
    });

    if (replacedRequestId) {
      const replacedConnection = this.getPendingConnection(replacedRequestId);
      if (replacedConnection) {
        await sendResponse(replacedConnection, createErrorResponse(replacedRequestId, 'invalid_auth', 'pairing_request_expired'));
        await replacedConnection.close();
      }
      this.pendingApprovalConnections.clear(replacedRequestId);
    }

    this.pendingApprovalConnections.set(request.requestId, connection.id, request.expiresAt, () => {
      this.expirePendingPairingRequests();
    });
  }

  private getPendingConnection(requestId: string): RemoteConnection | null {
    const connectionId = this.pendingApprovalConnections.get(requestId);
    return connectionId ? this.clientRegistry.getConnection(connectionId) : null;
  }

  private markConnectionSeen(connectionId: string, deviceId: string): void {
    this.clientRegistry.markSeen(connectionId, deviceId);
    this.notifyClientStatusChanged();
  }

  private markLastSeen(seenAt: number): void {
    this.options.onLastSeen?.(seenAt);
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

  private getPointerProfile(): PointerMovementProfile | null {
    try {
      return this.options.getPointerProfile?.() ?? null;
    } catch (error) {
      this.options.onError?.(error instanceof Error ? error.message : 'Pointer profile failed.');
      return null;
    }
  }

  private expirePendingPairingRequests(): void {
    const expired = this.options.pairingApprovalManager?.expirePendingRequests() ?? [];
    for (const request of expired) {
      const connection = this.getPendingConnection(request.requestId);
      if (connection) {
        void sendResponse(connection, createErrorResponse(request.requestId, 'invalid_auth', 'pairing_request_expired'));
        void connection.close();
      }
      this.pendingApprovalConnections.clear(request.requestId);
    }
  }

  private removePendingApprovalsForConnection(connectionId: string): void {
    for (const requestId of this.pendingApprovalConnections.clearForConnection(connectionId)) {
      this.options.pairingApprovalManager?.reject(requestId);
    }
  }

  private notifyClientStatusChanged(): void {
    this.options.onClientStatusChange?.();
  }
}
