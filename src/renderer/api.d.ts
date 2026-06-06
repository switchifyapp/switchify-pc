export {};

import type { PairingApprovalDecision, PendingPairingApprovalView } from '../shared/pairing-approval';
import type { ConnectionDetails, PairedDeviceView, PairingSessionView, PcServerStatus } from '../shared/server-status';

declare global {
  interface Window {
    switchifyPc: {
      appName: string;
      getServerStatus: () => Promise<PcServerStatus>;
      getPairingSession: () => Promise<PairingSessionView | null>;
      createPairingSession: () => Promise<PairingSessionView>;
      getConnectionDetails: () => Promise<ConnectionDetails>;
      getPairedDevices: () => Promise<PairedDeviceView[]>;
      disconnectClients: () => Promise<PcServerStatus>;
      getCursorOverlayEnabled: () => Promise<boolean>;
      setCursorOverlayEnabled: (enabled: boolean) => Promise<boolean>;
      getPendingPairingRequests: () => Promise<PendingPairingApprovalView[]>;
      respondToPairingRequest: (
        requestId: string,
        decision: PairingApprovalDecision
      ) => Promise<{ ok: boolean; reason?: string }>;
    };
  }
}
