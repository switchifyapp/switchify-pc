import { useCallback, useEffect, useMemo, useState } from 'react';
import QRCode from 'qrcode';
import { deriveDesktopUiState, type DesktopUiState } from '../shared/desktop-ui-state';
import type { PairingApprovalDecision, PendingPairingApprovalView } from '../shared/pairing-approval';
import { createPairingQrPayload } from '../shared/pairing-qr';
import type {
  ConnectionDetails,
  PairedDeviceView,
  PairingSessionView,
  PcConnectedClient,
  PcServerStatus
} from '../shared/server-status';
import type { CopyState } from './format';

export type SwitchifyPcStatusViewModel = {
  uiState: DesktopUiState;
  serverStatus: PcServerStatus | null;
  pairingSession: PairingSessionView | null;
  connectionDetails: ConnectionDetails | null;
  pairedDevices: PairedDeviceView[];
  pendingPairingRequests: PendingPairingApprovalView[];
  connectedClients: PcConnectedClient[];
  qrCodeUrl: string | null;
  connectionPayload: string;
  copyState: CopyState;
  isRefreshingPairing: boolean;
  showPairingWhileConnected: boolean;
  cursorOverlayEnabled: boolean;
  refresh: () => Promise<void>;
  refreshPairingCode: () => Promise<void>;
  disconnectClients: () => Promise<void>;
  connectAnotherPhone: () => Promise<void>;
  copyConnectionDetails: () => Promise<void>;
  toggleCursorOverlay: (enabled: boolean) => Promise<void>;
  respondToPairingRequest: (requestId: string, decision: PairingApprovalDecision) => Promise<void>;
};

export function useSwitchifyPcStatus(bridge: Window['switchifyPc']): SwitchifyPcStatusViewModel {
  const [serverStatus, setServerStatus] = useState<PcServerStatus | null>(null);
  const [pairingSession, setPairingSession] = useState<PairingSessionView | null>(null);
  const [connectionDetails, setConnectionDetails] = useState<ConnectionDetails | null>(null);
  const [pairedDevices, setPairedDevices] = useState<PairedDeviceView[]>([]);
  const [pendingPairingRequests, setPendingPairingRequests] = useState<PendingPairingApprovalView[]>([]);
  const [qrCodeUrl, setQrCodeUrl] = useState<string | null>(null);
  const [copyState, setCopyState] = useState<CopyState>('idle');
  const [isRefreshingPairing, setIsRefreshingPairing] = useState(false);
  const [showPairingWhileConnected, setShowPairingWhileConnected] = useState(false);
  const [cursorOverlayEnabled, setCursorOverlayEnabled] = useState(true);

  const refresh = useCallback(async (): Promise<void> => {
    const [status, session, details, devices, requests] = await Promise.all([
      bridge.getServerStatus(),
      bridge.getPairingSession(),
      bridge.getConnectionDetails(),
      bridge.getPairedDevices(),
      bridge.getPendingPairingRequests()
    ]);
    setServerStatus(status);
    setPairingSession(session);
    setConnectionDetails(details);
    setPairedDevices(devices);
    setPendingPairingRequests(requests);
  }, [bridge]);

  const refreshPairingCode = useCallback(async (): Promise<void> => {
    setIsRefreshingPairing(true);
    try {
      const session = await bridge.createPairingSession();
      setPairingSession(session);
      setConnectionDetails(await bridge.getConnectionDetails());
    } finally {
      setIsRefreshingPairing(false);
    }
  }, [bridge]);

  const disconnectClients = useCallback(async (): Promise<void> => {
    setServerStatus(await bridge.disconnectClients());
    setShowPairingWhileConnected(false);
  }, [bridge]);

  useEffect(() => {
    let cancelled = false;
    const load = async (): Promise<void> => {
      const overlayEnabled = await bridge.getCursorOverlayEnabled();
      if (!cancelled) {
        setCursorOverlayEnabled(overlayEnabled);
      }
      await refresh();
      if (cancelled) return;
      const session = await bridge.getPairingSession();
      if (!session && !cancelled) {
        await refreshPairingCode();
      }
    };

    void load();
    const interval = window.setInterval(() => {
      void refresh();
    }, 1000);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
    };
  }, [bridge, refresh, refreshPairingCode]);

  const toggleCursorOverlay = useCallback(
    async (enabled: boolean): Promise<void> => {
      setCursorOverlayEnabled(await bridge.setCursorOverlayEnabled(enabled));
    },
    [bridge]
  );

  useEffect(() => {
    if (!serverStatus?.connectedClientCount) {
      setShowPairingWhileConnected(false);
    }
  }, [serverStatus?.connectedClientCount]);

  const uiState = deriveDesktopUiState(serverStatus, connectionDetails, pairedDevices);

  const effectiveConnectionDetails = useMemo<ConnectionDetails | null>(() => {
    if (!connectionDetails) return null;
    return {
      ...connectionDetails,
      pairingCode: pairingSession?.pairingCode ?? connectionDetails.pairingCode,
      pairingNonce: pairingSession?.pairingNonce ?? connectionDetails.pairingNonce,
      expiresAt: pairingSession?.expiresAt ?? connectionDetails.expiresAt
    };
  }, [connectionDetails, pairingSession]);

  const pairingQrPayload = useMemo(() => createPairingQrPayload(effectiveConnectionDetails), [effectiveConnectionDetails]);

  const connectionPayload = useMemo(() => {
    if (!pairingQrPayload) return '';
    return JSON.stringify(pairingQrPayload, null, 2);
  }, [pairingQrPayload]);

  useEffect(() => {
    let cancelled = false;
    if (!pairingQrPayload) {
      setQrCodeUrl(null);
      return;
    }

    QRCode.toDataURL(JSON.stringify(pairingQrPayload), {
      errorCorrectionLevel: 'M',
      margin: 1,
      width: 280
    })
      .then((url) => {
        if (!cancelled) setQrCodeUrl(url);
      })
      .catch(() => {
        if (!cancelled) setQrCodeUrl(null);
      });

    return () => {
      cancelled = true;
    };
  }, [pairingQrPayload]);

  const copyConnectionDetails = useCallback(async (): Promise<void> => {
    if (!connectionPayload) return;
    try {
      await navigator.clipboard.writeText(connectionPayload);
      setCopyState('copied');
    } catch {
      setCopyState('failed');
    }
  }, [connectionPayload]);

  const connectAnotherPhone = useCallback(async (): Promise<void> => {
    setShowPairingWhileConnected(true);
    await refreshPairingCode();
  }, [refreshPairingCode]);

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
    pairingSession,
    connectionDetails,
    pairedDevices,
    pendingPairingRequests,
    connectedClients: serverStatus?.connectedClients ?? [],
    qrCodeUrl,
    connectionPayload,
    copyState,
    isRefreshingPairing,
    showPairingWhileConnected,
    cursorOverlayEnabled,
    refresh,
    refreshPairingCode,
    disconnectClients,
    connectAnotherPhone,
    copyConnectionDetails,
    toggleCursorOverlay,
    respondToPairingRequest
  };
}
