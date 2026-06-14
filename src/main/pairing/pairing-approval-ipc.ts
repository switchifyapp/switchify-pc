import { ipcMain } from 'electron';
import {
  GET_PENDING_PAIRING_REQUESTS_CHANNEL,
  RESPOND_TO_PAIRING_REQUEST_CHANNEL
} from '../../shared/ipc-channels';
import type { PairingApprovalDecision } from '../../shared/pairing-approval';
import type { ControlService } from '../control/control-service';

export function registerPairingApprovalIpc(controlService: ControlService): void {
  ipcMain.handle(GET_PENDING_PAIRING_REQUESTS_CHANNEL, () => controlService.getPendingPairingRequests());
  ipcMain.handle(RESPOND_TO_PAIRING_REQUEST_CHANNEL, (_event, requestId: string, decision: PairingApprovalDecision) =>
    controlService.respondToPairingRequest(requestId, decision)
  );
}
