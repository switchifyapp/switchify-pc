import { useCallback, useEffect, useState } from 'react';
import { deriveDesktopUiState, type DesktopUiState } from '../shared/desktop-ui-state';
import type { PairingApprovalDecision, PendingPairingApprovalView } from '../shared/pairing-approval';
import type {
  ConnectionDetails,
  PairedDeviceView,
  PcConnectedClient,
  PcServerStatus
} from '../shared/server-status';

export type SwitchifyPcStatusViewModel = {
  uiState: DesktopUiState;
  serverStatus: PcServerStatus | null;
  connectionDetails: ConnectionDetails | null;
  pairedDevices: PairedDeviceView[];
  pendingPairingRequests: PendingPairingApprovalView[];
  connectedClients: PcConnectedClient[];
  cursorOverlayEnabled: boolean;
  refresh: () => Promise<void>;
  disconnectClients: () => Promise<void>;
  toggleCursorOverlay: (enabled: boolean) => Promise<void>;
  respondToPairingRequest: (requestId: string, decision: PairingApprovalDecision) => Promise<void>;
};

export function useSwitchifyPcStatus(bridge: Window['switchifyPc']): SwitchifyPcStatusViewModel {
  const [serverStatus, setServerStatus] = useState<PcServerStatus | null>(null);
  const [connectionDetails, setConnectionDetails] = useState<ConnectionDetails | null>(null);
  const [pairedDevices, setPairedDevices] = useState<PairedDeviceView[]>([]);
  const [pendingPairingRequests, setPendingPairingRequests] = useState<PendingPairingApprovalView[]>([]);
  const [cursorOverlayEnabled, setCursorOverlayEnabled] = useState(true);

  const refresh = useCallback(async (): Promise<void> => {
    const [status, details, devices, requests] = await Promise.all([
      bridge.getServerStatus(),
      bridge.getConnectionDetails(),
      bridge.getPairedDevices(),
      bridge.getPendingPairingRequests()
    ]);
    setServerStatus(status);
    setConnectionDetails(details);
    setPairedDevices(devices);
    setPendingPairingRequests(requests);
  }, [bridge]);

  const disconnectClients = useCallback(async (): Promise<void> => {
    setServerStatus(await bridge.disconnectClients());
  }, [bridge]);

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
    connectedClients: serverStatus?.connectedClients ?? [],
    cursorOverlayEnabled,
    refresh,
    disconnectClients,
    toggleCursorOverlay,
    respondToPairingRequest
  };
}
