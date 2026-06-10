export {};

import type { FirewallDiagnostics, FirewallRepairResult } from '../shared/firewall';
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
      forgetPairedDevice: (
        deviceId: string
      ) => Promise<
        { ok: true; pairedDevices: PairedDeviceView[]; status: PcServerStatus } | { ok: false; reason: string }
      >;
      getCursorOverlayEnabled: () => Promise<boolean>;
      setCursorOverlayEnabled: (enabled: boolean) => Promise<boolean>;
      openSettingsWindow: () => Promise<void>;
      getFirewallDiagnostics: () => Promise<FirewallDiagnostics>;
      repairFirewall: () => Promise<FirewallRepairResult>;
      getPendingPairingRequests: () => Promise<PendingPairingApprovalView[]>;
      respondToPairingRequest: (
        requestId: string,
        decision: PairingApprovalDecision
      ) => Promise<{ ok: boolean; reason?: string }>;
    };
  }
}
