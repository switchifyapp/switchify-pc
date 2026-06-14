import type { BluetoothStatus } from '../../shared/bluetooth-status';
import type { PairingApprovalDecision, PendingPairingApprovalView } from '../../shared/pairing-approval';
import type { CommandRequest, PointerMovementProfile } from '../../shared/protocol';
import type { PcConnectedClient, PcControlStatus, PcControlStatusListener } from '../../shared/server-status';
import type { CommandAuthValidator } from '../pairing/auth';
import type { PairingApprovalManager } from '../pairing/pairing-approval-manager';
import type { PairingManager } from '../pairing/pairing-manager';
import type { RemoteConnection } from '../transport/remote-connection';
import { RemoteSessionManager, type CommandHandlerResult } from '../transport/remote-session-manager';
import { cloneControlStatus, createInitialControlStatus } from './control-status';

export type ControlServiceOptions = {
  pairingManager: PairingManager;
  pairingApprovalManager?: PairingApprovalManager;
  authValidator: CommandAuthValidator;
  getPointerProfile?: () => PointerMovementProfile;
  onStatusChange?: PcControlStatusListener;
  onCommand?: (command: CommandRequest) => Promise<CommandHandlerResult> | CommandHandlerResult;
};

export class ControlService {
  private readonly sessions: RemoteSessionManager;
  private status: PcControlStatus;

  constructor(private readonly options: ControlServiceOptions) {
    this.status = createInitialControlStatus();
    this.sessions = new RemoteSessionManager({
      pairingManager: options.pairingManager,
      pairingApprovalManager: options.pairingApprovalManager,
      authValidator: options.authValidator,
      getPointerProfile: options.getPointerProfile,
      onCommand: options.onCommand,
      onClientStatusChange: () => this.updateClientStatus(),
      onLastSeen: (seenAt) => this.setStatus({ lastSeenAt: seenAt, lastError: null }),
      onError: (message) => this.setStatus({ lastError: message })
    });
  }

  getStatus(): PcControlStatus {
    return cloneControlStatus(this.status);
  }

  addRemoteConnection(connection: RemoteConnection): PcConnectedClient {
    return this.sessions.addConnection(connection);
  }

  async handleRemoteMessage(connectionId: string, rawMessage: string): Promise<void> {
    await this.sessions.handleMessage(connectionId, rawMessage);
  }

  removeRemoteConnection(connectionId: string): void {
    this.sessions.removeConnection(connectionId);
  }

  async closeAllConnections(): Promise<PcControlStatus> {
    await this.sessions.closeAllConnections();
    return this.getStatus();
  }

  disconnectClients(): PcControlStatus {
    void this.sessions.closeAllConnections();
    return this.getStatus();
  }

  disconnectDevice(deviceId: string): PcControlStatus {
    void this.sessions.disconnectDevice(deviceId);
    return this.getStatus();
  }

  getPendingPairingRequests(): PendingPairingApprovalView[] {
    return this.sessions.getPendingPairingRequests();
  }

  async respondToPairingRequest(
    requestId: string,
    decision: PairingApprovalDecision
  ): Promise<{ ok: boolean; reason?: string }> {
    return this.sessions.respondToPairingRequest(requestId, decision);
  }

  setBluetoothStatus(bluetooth: BluetoothStatus): PcControlStatus {
    this.setStatus({ bluetooth, state: controlStateFromBluetooth(bluetooth) });
    return this.getStatus();
  }

  setDesktopId(desktopId: string): PcControlStatus {
    this.setStatus({ desktopId });
    return this.getStatus();
  }

  private updateClientStatus(): void {
    this.setStatus({
      connectedClientCount: this.sessions.getAuthenticatedClientCount(),
      connectedClients: this.sessions.getAuthenticatedClients()
    });
  }

  private setStatus(update: Partial<PcControlStatus>): void {
    this.status = { ...this.status, ...update };
    this.options.onStatusChange?.(this.getStatus());
  }
}

function controlStateFromBluetooth(bluetooth: BluetoothStatus): PcControlStatus['state'] {
  if (bluetooth.status === 'starting') return 'starting';
  if (bluetooth.status === 'ready' || bluetooth.status === 'connected') return 'ready';
  if (bluetooth.status === 'unavailable' || bluetooth.status === 'error') return 'error';
  return 'stopped';
}
