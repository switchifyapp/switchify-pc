import { useCallback, useEffect, useState } from 'react';
import { deriveDesktopUiState, type DesktopUiState } from '../shared/desktop-ui-state';
import type { FirewallDiagnostics, FirewallRepairResult } from '../shared/firewall';
import type { PairingApprovalDecision, PendingPairingApprovalView } from '../shared/pairing-approval';
import type {
  ConnectionDetails,
  PairedDeviceView,
  PcServerStatus
} from '../shared/server-status';
import { toConnectedDeviceViews, type ConnectedDeviceView } from './connected-devices';

export type SwitchifyPcStatusViewModel = {
  uiState: DesktopUiState;
  serverStatus: PcServerStatus | null;
  connectionDetails: ConnectionDetails | null;
  pairedDevices: PairedDeviceView[];
  pendingPairingRequests: PendingPairingApprovalView[];
  connectedDevices: ConnectedDeviceView[];
  cursorOverlayEnabled: boolean;
  firewallDiagnostics: FirewallDiagnostics | null;
  refresh: () => Promise<void>;
  refreshFirewallDiagnostics: () => Promise<void>;
  disconnectClients: () => Promise<void>;
  forgetPairedDevice: (deviceId: string) => Promise<{ ok: boolean; reason?: string }>;
  repairFirewall: () => Promise<FirewallRepairResult>;
  toggleCursorOverlay: (enabled: boolean) => Promise<void>;
  respondToPairingRequest: (requestId: string, decision: PairingApprovalDecision) => Promise<void>;
};

export function useSwitchifyPcStatus(bridge: Window['switchifyPc']): SwitchifyPcStatusViewModel {
  const [serverStatus, setServerStatus] = useState<PcServerStatus | null>(null);
  const [connectionDetails, setConnectionDetails] = useState<ConnectionDetails | null>(null);
  const [pairedDevices, setPairedDevices] = useState<PairedDeviceView[]>([]);
  const [pendingPairingRequests, setPendingPairingRequests] = useState<PendingPairingApprovalView[]>([]);
  const [cursorOverlayEnabled, setCursorOverlayEnabled] = useState(true);
  const [firewallDiagnostics, setFirewallDiagnostics] = useState<FirewallDiagnostics | null>(null);

  const refresh = useCallback(async (): Promise<void> => {
    const [status, details, devices, requests, firewall] = await Promise.all([
      bridge.getServerStatus(),
      bridge.getConnectionDetails(),
      bridge.getPairedDevices(),
      bridge.getPendingPairingRequests(),
      bridge.getFirewallDiagnostics()
    ]);
    setServerStatus(status);
    setConnectionDetails(details);
    setPairedDevices(devices);
    setPendingPairingRequests(requests);
    setFirewallDiagnostics(firewall);
  }, [bridge]);

  const refreshFirewallDiagnostics = useCallback(async (): Promise<void> => {
    setFirewallDiagnostics(await bridge.getFirewallDiagnostics());
  }, [bridge]);

  const disconnectClients = useCallback(async (): Promise<void> => {
    setServerStatus(await bridge.disconnectClients());
  }, [bridge]);

  const forgetPairedDevice = useCallback(
    async (deviceId: string): Promise<{ ok: boolean; reason?: string }> => {
      const result = await bridge.forgetPairedDevice(deviceId);
      if (!result.ok) {
        return result;
      }

      setPairedDevices(result.pairedDevices);
      setServerStatus(result.status);
      await refresh();
      return { ok: true };
    },
    [bridge, refresh]
  );

  const repairFirewall = useCallback(async (): Promise<FirewallRepairResult> => {
    const result = await bridge.repairFirewall();
    if (result.diagnostics) {
      setFirewallDiagnostics(result.diagnostics);
    } else {
      await refreshFirewallDiagnostics();
    }
    return result;
  }, [bridge, refreshFirewallDiagnostics]);

  useEffect(() => {
    let cancelled = false;
    const load = async (): Promise<void> => {
      const overlayEnabled = await bridge.getCursorOverlayEnabled();
      if (!cancelled) {
        setCursorOverlayEnabled(overlayEnabled);
      }
      await refresh();
    };

    void load();
    const interval = window.setInterval(() => {
      void refresh();
    }, 1000);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [bridge, refresh]);

  const toggleCursorOverlay = useCallback(
    async (enabled: boolean): Promise<void> => {
      setCursorOverlayEnabled(await bridge.setCursorOverlayEnabled(enabled));
    },
    [bridge]
  );

  const uiState = deriveDesktopUiState(serverStatus, connectionDetails, pairedDevices);

  const respondToPairingRequest = useCallback(
    async (requestId: string, decision: PairingApprovalDecision): Promise<void> => {
      await bridge.respondToPairingRequest(requestId, decision);
      await refresh();
    },
    [bridge, refresh]
  );

  return {
    uiState,
    serverStatus,
    connectionDetails,
    pairedDevices,
    pendingPairingRequests,
    connectedDevices: toConnectedDeviceViews(serverStatus?.connectedClients ?? [], pairedDevices),
    cursorOverlayEnabled,
    firewallDiagnostics,
    refresh,
    refreshFirewallDiagnostics,
    disconnectClients,
    forgetPairedDevice,
    repairFirewall,
    toggleCursorOverlay,
    respondToPairingRequest
  };
}
