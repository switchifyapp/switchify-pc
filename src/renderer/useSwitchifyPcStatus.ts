import { useCallback, useEffect, useState } from 'react';
import {
  DEFAULT_CURSOR_OVERLAY_SETTINGS,
  normalizeCursorOverlaySettings,
  type CursorOverlaySettings
} from '../shared/cursor-overlay-settings';
import { deriveDesktopUiState, type DesktopUiState } from '../shared/desktop-ui-state';
import type { PairingApprovalDecision, PendingPairingApprovalView } from '../shared/pairing-approval';
import type { PairedDeviceView, PcControlStatus } from '../shared/server-status';
import { toConnectedDeviceViews, type ConnectedDeviceView } from './connected-devices';

export type SwitchifyPcStatusViewModel = {
  uiState: DesktopUiState;
  serverStatus: PcControlStatus | null;
  pairedDevices: PairedDeviceView[];
  pendingPairingRequests: PendingPairingApprovalView[];
  connectedDevices: ConnectedDeviceView[];
  cursorOverlaySettings: CursorOverlaySettings;
  refresh: () => Promise<void>;
  disconnectClients: () => Promise<void>;
  forgetPairedDevice: (deviceId: string) => Promise<{ ok: boolean; reason?: string }>;
  updateCursorOverlaySettings: (settings: CursorOverlaySettings) => Promise<void>;
  respondToPairingRequest: (requestId: string, decision: PairingApprovalDecision) => Promise<void>;
};

export function useSwitchifyPcStatus(bridge: Window['switchifyPc']): SwitchifyPcStatusViewModel {
  const [serverStatus, setServerStatus] = useState<PcControlStatus | null>(null);
  const [pairedDevices, setPairedDevices] = useState<PairedDeviceView[]>([]);
  const [pendingPairingRequests, setPendingPairingRequests] = useState<PendingPairingApprovalView[]>([]);
  const [cursorOverlaySettings, setCursorOverlaySettings] = useState<CursorOverlaySettings>(
    DEFAULT_CURSOR_OVERLAY_SETTINGS
  );

  const refresh = useCallback(async (): Promise<void> => {
    const [status, devices, requests] = await Promise.all([
      bridge.getServerStatus(),
      bridge.getPairedDevices(),
      bridge.getPendingPairingRequests()
    ]);
    setServerStatus(status);
    setPairedDevices(devices);
    setPendingPairingRequests(requests);
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

  useEffect(() => {
    let cancelled = false;
    const load = async (): Promise<void> => {
      const overlayEnabled = await bridge.getCursorOverlayEnabled();
      const overlaySettings = await bridge.getCursorOverlaySettings();
      if (!cancelled) {
        setCursorOverlaySettings(normalizeCursorOverlaySettings({ ...overlaySettings, enabled: overlayEnabled }));
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

  const updateCursorOverlaySettings = useCallback(
    async (settings: CursorOverlaySettings): Promise<void> => {
      setCursorOverlaySettings(await bridge.setCursorOverlaySettings(settings));
    },
    [bridge]
  );

  const uiState = deriveDesktopUiState(serverStatus, pairedDevices);

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
    pairedDevices,
    pendingPairingRequests,
    connectedDevices: toConnectedDeviceViews(serverStatus?.connectedClients ?? [], pairedDevices),
    cursorOverlaySettings,
    refresh,
    disconnectClients,
    forgetPairedDevice,
    updateCursorOverlaySettings,
    respondToPairingRequest
  };
}
