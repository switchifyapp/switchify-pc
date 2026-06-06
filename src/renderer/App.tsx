import type { ReactElement } from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';
import QRCode from 'qrcode';
import { deriveDesktopUiState, type DesktopUiState } from '../shared/desktop-ui-state';
import { createPairingQrPayload } from '../shared/pairing-qr';
import type {
  ConnectionDetails,
  PairedDeviceView,
  PairingSessionView,
  PcConnectedClient,
  PcServerStatus
} from '../shared/server-status';

type CopyState = 'idle' | 'copied' | 'failed';

export function App(): ReactElement {
  const bridge = window.switchifyPc;
  const [serverStatus, setServerStatus] = useState<PcServerStatus | null>(null);
  const [pairingSession, setPairingSession] = useState<PairingSessionView | null>(null);
  const [connectionDetails, setConnectionDetails] = useState<ConnectionDetails | null>(null);
  const [pairedDevices, setPairedDevices] = useState<PairedDeviceView[]>([]);
  const [qrCodeUrl, setQrCodeUrl] = useState<string | null>(null);
  const [copyState, setCopyState] = useState<CopyState>('idle');
  const [isRefreshingPairing, setIsRefreshingPairing] = useState(false);
  const [showPairingWhileConnected, setShowPairingWhileConnected] = useState(false);

  const refresh = useCallback(async (): Promise<void> => {
    const [status, session, details, devices] = await Promise.all([
      bridge.getServerStatus(),
      bridge.getPairingSession(),
      bridge.getConnectionDetails(),
      bridge.getPairedDevices()
    ]);
    setServerStatus(status);
    setPairingSession(session);
    setConnectionDetails(details);
    setPairedDevices(devices);
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

  return (
    <main className="app-shell">
      <section className="setup-card" aria-label="Switchify PC setup">
        <StatusHeader state={uiState} appName={bridge.appName} />

        <PrimaryContent
          state={uiState}
          connectedClients={serverStatus?.connectedClients ?? []}
          showPairingWhileConnected={showPairingWhileConnected}
          qrCodeUrl={qrCodeUrl}
          isRefreshingPairing={isRefreshingPairing}
          onRefreshPairing={refreshPairingCode}
          onDisconnect={disconnectClients}
          onConnectAnotherPhone={connectAnotherPhone}
          onRefresh={refresh}
        />

        <TroubleshootingDetails
          serverStatus={serverStatus}
          connectionDetails={connectionDetails}
          pairingSession={pairingSession}
          pairedDevices={pairedDevices}
          connectedClients={serverStatus?.connectedClients ?? []}
          connectionPayload={connectionPayload}
          copyState={copyState}
          onCopy={copyConnectionDetails}
          onDisconnect={disconnectClients}
        />
      </section>
    </main>
  );
}

function StatusHeader({ state, appName }: { state: DesktopUiState; appName: string }): ReactElement {
  return (
    <header className="setup-header">
      <div>
        <p className="section-label">Desktop companion</p>
        <h1>{appName}</h1>
      </div>
      <div className={`status-badge status-badge-${statusTone(state)}`}>
        <span className="status-dot" />
        {statusBadgeLabel(state)}
      </div>
    </header>
  );
}

function PrimaryContent({
  state,
  connectedClients,
  showPairingWhileConnected,
  qrCodeUrl,
  isRefreshingPairing,
  onRefreshPairing,
  onDisconnect,
  onConnectAnotherPhone,
  onRefresh
}: {
  state: DesktopUiState;
  connectedClients: PcConnectedClient[];
  showPairingWhileConnected: boolean;
  qrCodeUrl: string | null;
  isRefreshingPairing: boolean;
  onRefreshPairing: () => Promise<void>;
  onDisconnect: () => Promise<void>;
  onConnectAnotherPhone: () => Promise<void>;
  onRefresh: () => Promise<void>;
}): ReactElement {
  if (state === 'loading' || state === 'starting' || state === 'not-running') {
    return <StartingState state={state} />;
  }

  if (state === 'server-error') {
    return <ServerErrorState onRefresh={onRefresh} />;
  }

  if (state === 'connected') {
    return (
      <ConnectedReadyState
        connectedClients={connectedClients}
        showPairing={showPairingWhileConnected}
        qrCodeUrl={qrCodeUrl}
        isRefreshingPairing={isRefreshingPairing}
        onRefreshPairing={onRefreshPairing}
        onDisconnect={onDisconnect}
        onConnectAnotherPhone={onConnectAnotherPhone}
      />
    );
  }

  if (state === 'waiting-for-phone') {
    return (
      <WaitingForPhoneState
        qrCodeUrl={qrCodeUrl}
        isRefreshingPairing={isRefreshingPairing}
        onRefreshPairing={onRefreshPairing}
      />
    );
  }

  return <PairingSetup qrCodeUrl={qrCodeUrl} isRefreshingPairing={isRefreshingPairing} onRefreshPairing={onRefreshPairing} />;
}

function StartingState({ state }: { state: 'loading' | 'starting' | 'not-running' }): ReactElement {
  if (state === 'not-running') {
    return (
      <section className="primary-state">
        <h2>Switchify PC is not running</h2>
        <p>Restart the app to connect your phone.</p>
      </section>
    );
  }

  return (
    <section className="primary-state">
      <h2>{state === 'starting' ? 'Starting Switchify PC...' : 'Getting things ready...'}</h2>
      <p>Switchify PC is preparing your phone connection.</p>
      <div className="qr-code-frame qr-placeholder">Preparing your code...</div>
    </section>
  );
}

function ServerErrorState({ onRefresh }: { onRefresh: () => Promise<void> }): ReactElement {
  return (
    <section className="primary-state">
      <h2>Switchify PC needs attention</h2>
      <p>Restart Switchify PC. If this keeps happening, open troubleshooting details.</p>
      <div className="action-row">
        <button type="button" className="primary-button" onClick={() => void onRefresh()}>
          Refresh
        </button>
      </div>
    </section>
  );
}

function PairingSetup({
  qrCodeUrl,
  isRefreshingPairing,
  onRefreshPairing
}: {
  qrCodeUrl: string | null;
  isRefreshingPairing: boolean;
  onRefreshPairing: () => Promise<void>;
}): ReactElement {
  return (
    <section className="primary-state pairing-state" aria-labelledby="pairing-heading">
      <h2 id="pairing-heading">Connect your phone</h2>
      <p>Open Switchify on your phone and scan this code.</p>
      <QrCodeFrame qrCodeUrl={qrCodeUrl} />
      <div className="action-row centered">
        <button type="button" className="primary-button" onClick={() => void onRefreshPairing()} disabled={isRefreshingPairing}>
          {isRefreshingPairing ? 'Refreshing...' : 'Refresh code'}
        </button>
      </div>
      <p className="helper-text">This code refreshes for your security.</p>
    </section>
  );
}

function ConnectedReadyState({
  connectedClients,
  showPairing,
  qrCodeUrl,
  isRefreshingPairing,
  onRefreshPairing,
  onDisconnect,
  onConnectAnotherPhone
}: {
  connectedClients: PcConnectedClient[];
  showPairing: boolean;
  qrCodeUrl: string | null;
  isRefreshingPairing: boolean;
  onRefreshPairing: () => Promise<void>;
  onDisconnect: () => Promise<void>;
  onConnectAnotherPhone: () => Promise<void>;
}): ReactElement {
  return (
    <section className="primary-state">
      <h2>Your phone is connected</h2>
      <p>You can control this PC from Switchify.</p>
      <DeviceSummary clients={connectedClients} />
      <div className="action-row centered">
        <button type="button" className="primary-button" onClick={() => void onDisconnect()}>
          Disconnect phone
        </button>
        <button type="button" onClick={() => void onConnectAnotherPhone()}>
          Connect another phone
        </button>
      </div>
      {showPairing ? (
        <div className="connected-pairing">
          <PairingSetup qrCodeUrl={qrCodeUrl} isRefreshingPairing={isRefreshingPairing} onRefreshPairing={onRefreshPairing} />
        </div>
      ) : null}
    </section>
  );
}

function WaitingForPhoneState({
  qrCodeUrl,
  isRefreshingPairing,
  onRefreshPairing
}: {
  qrCodeUrl: string | null;
  isRefreshingPairing: boolean;
  onRefreshPairing: () => Promise<void>;
}): ReactElement {
  return (
    <section className="primary-state">
      <h2>Waiting for your phone</h2>
      <p>Open Switchify on your phone to reconnect, or scan a new code to pair another phone.</p>
      <PairingSetup qrCodeUrl={qrCodeUrl} isRefreshingPairing={isRefreshingPairing} onRefreshPairing={onRefreshPairing} />
    </section>
  );
}

function QrCodeFrame({ qrCodeUrl }: { qrCodeUrl: string | null }): ReactElement {
  return (
    <div className="qr-code-frame" aria-label="Scan to pair">
      {qrCodeUrl ? <img src={qrCodeUrl} alt="Switchify PC pairing QR code" /> : <span>Preparing your code...</span>}
    </div>
  );
}

function DeviceSummary({ clients }: { clients: PcConnectedClient[] }): ReactElement {
  const primaryClient = clients[0];
  return (
    <div className="device-summary">
      <strong>{primaryClient?.deviceId ?? 'Phone connected'}</strong>
      {clients.length > 1 ? <span>{clients.length} phones connected.</span> : <span>Connected now.</span>}
    </div>
  );
}

function TroubleshootingDetails({
  serverStatus,
  connectionDetails,
  pairingSession,
  pairedDevices,
  connectedClients,
  connectionPayload,
  copyState,
  onCopy,
  onDisconnect
}: {
  serverStatus: PcServerStatus | null;
  connectionDetails: ConnectionDetails | null;
  pairingSession: PairingSessionView | null;
  pairedDevices: PairedDeviceView[];
  connectedClients: PcConnectedClient[];
  connectionPayload: string;
  copyState: CopyState;
  onCopy: () => Promise<void>;
  onDisconnect: () => Promise<void>;
}): ReactElement {
  return (
    <details className="troubleshooting-panel">
      <summary>Troubleshooting</summary>
      <div className="troubleshooting-content">
        <ManualPairingFallback
          pairingSession={pairingSession}
          connectionDetails={connectionDetails}
          connectionPayload={connectionPayload}
          copyState={copyState}
          onCopy={onCopy}
        />
        <TroubleshootingSection title="Connected phones">
          <ClientList clients={connectedClients} />
          <button type="button" onClick={() => void onDisconnect()} disabled={connectedClients.length === 0}>
            Disconnect phone
          </button>
        </TroubleshootingSection>
        <TroubleshootingSection title="Saved phones">
          <PairedDeviceList devices={pairedDevices} />
        </TroubleshootingSection>
        <TroubleshootingSection title="Server details">
          <DetailGrid>
            <DetailItem label="Server status" value={serverStatus?.state ?? 'Unknown'} />
            <DetailItem label="Port" value={serverStatus ? String(serverStatus.port) : 'Unknown'} />
            <DetailItem label="Last command" value={formatTimestamp(serverStatus?.lastSeenAt ?? null)} />
            <DetailItem label="Recent error" value={serverStatus?.lastError ?? 'No recent errors.'} />
          </DetailGrid>
        </TroubleshootingSection>
      </div>
    </details>
  );
}

function ManualPairingFallback({
  pairingSession,
  connectionDetails,
  connectionPayload,
  copyState,
  onCopy
}: {
  pairingSession: PairingSessionView | null;
  connectionDetails: ConnectionDetails | null;
  connectionPayload: string;
  copyState: CopyState;
  onCopy: () => Promise<void>;
}): ReactElement {
  return (
    <TroubleshootingSection title="Manual pairing code">
      <DetailGrid>
        <DetailItem label="Pairing code" value={pairingSession?.pairingCode ?? 'No active code.'} />
        <DetailItem label="Connection address" value={connectionDetails?.websocketUrl ?? 'Not available.'} />
        <DetailItem label="Desktop id" value={connectionDetails?.desktopId ?? 'Not available.'} />
        <DetailItem label="Expires" value={formatExpiry(pairingSession?.expiresAt ?? null)} />
      </DetailGrid>
      <ConnectionUrlList urls={connectionDetails?.websocketUrls ?? []} primaryUrl={connectionDetails?.websocketUrl ?? null} />
      <div className="copy-row">
        <button type="button" onClick={() => void onCopy()} disabled={!connectionPayload}>
          Copy manual setup details
        </button>
        <span>{formatCopyState(copyState)}</span>
      </div>
    </TroubleshootingSection>
  );
}

function TroubleshootingSection({ title, children }: { title: string; children: ReactElement | ReactElement[] }): ReactElement {
  return (
    <section className="troubleshooting-section">
      <h3>{title}</h3>
      {children}
    </section>
  );
}

function DetailGrid({ children }: { children: ReactElement | ReactElement[] }): ReactElement {
  return <div className="detail-grid">{children}</div>;
}

function DetailItem({ label, value }: { label: string; value: string }): ReactElement {
  return (
    <div className="detail-item">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function ClientList({ clients }: { clients: PcConnectedClient[] }): ReactElement {
  if (clients.length === 0) {
    return <div className="empty-state">No phones connected.</div>;
  }

  return (
    <ul className="technical-list">
      {clients.map((client) => (
        <li key={client.id}>
          <strong>{client.deviceId ?? 'Unidentified phone'}</strong>
          <span>{client.remoteAddress ?? 'Unknown address'}</span>
        </li>
      ))}
    </ul>
  );
}

function PairedDeviceList({ devices }: { devices: PairedDeviceView[] }): ReactElement {
  if (devices.length === 0) {
    return <div className="empty-state">No phones saved.</div>;
  }

  return (
    <ul className="technical-list">
      {devices.map((device) => (
        <li key={device.deviceId}>
          <strong>{device.deviceName}</strong>
          <span>{device.deviceId}</span>
          <span>Paired {formatTimestamp(device.pairedAt)}</span>
          <span>Last seen {formatTimestamp(device.lastSeenAt)}</span>
        </li>
      ))}
    </ul>
  );
}

function ConnectionUrlList({ urls, primaryUrl }: { urls: string[]; primaryUrl: string | null }): ReactElement | null {
  if (urls.length === 0) return null;

  return (
    <ul className="connection-url-list" aria-label="Connection addresses">
      {urls.map((url) => (
        <li key={url}>
          <span>{url === primaryUrl ? 'Primary' : isLoopbackUrl(url) ? 'Local test' : 'Alternate'}</span>
          <strong>{url}</strong>
        </li>
      ))}
    </ul>
  );
}

function statusTone(state: DesktopUiState): 'ready' | 'connected' | 'waiting' | 'error' {
  if (state === 'connected') return 'connected';
  if (state === 'server-error') return 'error';
  if (state === 'loading' || state === 'starting' || state === 'waiting-for-phone' || state === 'not-running') return 'waiting';
  return 'ready';
}

function statusBadgeLabel(state: DesktopUiState): string {
  if (state === 'connected') return 'Connected';
  if (state === 'server-error') return 'Needs attention';
  if (state === 'waiting-for-phone') return 'Waiting';
  if (state === 'not-running') return 'Not running';
  if (state === 'loading' || state === 'starting') return 'Starting...';
  return 'Ready';
}

function formatExpiry(value: number | null): string {
  if (!value) return 'No active code.';
  const remainingMs = Math.max(0, value - Date.now());
  const minutes = Math.floor(remainingMs / 60_000);
  const seconds = Math.floor((remainingMs % 60_000) / 1000);
  return `${minutes}m ${seconds.toString().padStart(2, '0')}s`;
}

function formatTimestamp(value: number | null): string {
  if (!value) return 'Not yet.';
  return new Intl.DateTimeFormat(undefined, {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  }).format(value);
}

function formatCopyState(state: CopyState): string {
  if (state === 'copied') return 'Copied.';
  if (state === 'failed') return 'Copy failed.';
  return '';
}

function isLoopbackUrl(url: string): boolean {
  return url.includes('://127.0.0.1:') || url.includes('://localhost:');
}
