import { ipcMain } from 'electron';
import {
  GET_PENDING_PAIRING_REQUESTS_CHANNEL,
  RESPOND_TO_PAIRING_REQUEST_CHANNEL
} from '../../shared/ipc-channels';
import type { PairingApprovalDecision } from '../../shared/pairing-approval';
import type { PcWebSocketServer } from '../websocket/server';

export function registerPairingApprovalIpc(server: PcWebSocketServer): void {
  ipcMain.handle(GET_PENDING_PAIRING_REQUESTS_CHANNEL, () => server.getPendingPairingRequests());
  ipcMain.handle(RESPOND_TO_PAIRING_REQUEST_CHANNEL, (_event, requestId: string, decision: PairingApprovalDecision) =>
    server.respondToPairingRequest(requestId, decision)
  );
}
