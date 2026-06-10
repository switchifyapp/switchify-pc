export {};

import type { PairingApprovalDecision, PendingPairingApprovalView } from '../shared/pairing-approval';
import type { ConnectionDetails, PairedDeviceView, PcServerStatus } from '../shared/server-status';

declare global {
  interface Window {
    switchifyPc: {
      appName: string;
      getServerStatus: () => Promise<PcServerStatus>;
      getConnectionDetails: () => Promise<ConnectionDetails>;
      getPairedDevices: () => Promise<PairedDeviceView[]>;
      disconnectClients: () => Promise<PcServerStatus>;
      getCursorOverlayEnabled: () => Promise<boolean>;
      setCursorOverlayEnabled: (enabled: boolean) => Promise<boolean>;
      openSettingsWindow: () => Promise<void>;
      getPendingPairingRequests: () => Promise<PendingPairingApprovalView[]>;
      respondToPairingRequest: (
        requestId: string,
        decision: PairingApprovalDecision
      ) => Promise<{ ok: boolean; reason?: string }>;
    };
  }
}
