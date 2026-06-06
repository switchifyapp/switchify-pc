import type { ReactElement } from 'react';
import { useCallback, useEffect, useMemo, useState } from 'react';
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
  const [copyState, setCopyState] = useState<CopyState>('idle');

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
    const session = await bridge.createPairingSession();
    setPairingSession(session);
    setConnectionDetails(await bridge.getConnectionDetails());
  }, [bridge]);

  const disconnectClients = useCallback(async (): Promise<void> => {
    setServerStatus(await bridge.disconnectClients());
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

  const connectionPayload = useMemo(() => {
    if (!connectionDetails) return '';
    return JSON.stringify(
      {
        desktopId: connectionDetails.desktopId,
        websocketUrl: connectionDetails.websocketUrl,
        websocketUrls: connectionDetails.websocketUrls,
        pairingCode: connectionDetails.pairingCode,
        pairingNonce: connectionDetails.pairingNonce,
        expiresAt: connectionDetails.expiresAt
      },
      null,
      2
    );
  }, [connectionDetails]);

  const copyConnectionDetails = useCallback(async (): Promise<void> => {
    if (!connectionPayload) return;
    try {
      await navigator.clipboard.writeText(connectionPayload);
      setCopyState('copied');
    } catch {
      setCopyState('failed');
    }
  }, [connectionPayload]);

  return (
    <main className="app-shell">
      <header className="top-bar">
        <div>
          <div className="eyebrow">Desktop companion</div>
          <h1>{bridge.appName}</h1>
        </div>
        <div className={`status-pill status-pill-${serverStatus?.state ?? 'starting'}`}>
          <span className="status-dot" />
          {formatServerStatus(serverStatus)}
        </div>
      </header>

      <section className="dashboard-grid" aria-label="Switchify PC status">
        <section className="panel pairing-panel" aria-labelledby="pairing-title">
          <div className="panel-heading">
            <div>
              <p className="section-label">Pairing</p>
              <h2 id="pairing-title">Pairing code</h2>
            </div>
            <button type="button" onClick={refreshPairingCode}>
              Refresh
            </button>
          </div>

          <div className="pairing-code" aria-label="Current pairing code">
            {pairingSession?.pairingCode ?? '------'}
          </div>
          <div className="meta-grid">
            <MetaItem label="Desktop id" value={connectionDetails?.desktopId ?? 'Loading.'} />
            <MetaItem label="Android address" value={connectionDetails?.websocketUrl ?? 'Loading.'} />
            <MetaItem label="Expires" value={formatExpiry(pairingSession?.expiresAt ?? null)} />
          </div>
          <ConnectionUrlList urls={connectionDetails?.websocketUrls ?? []} primaryUrl={connectionDetails?.websocketUrl ?? null} />
          <div className="copy-row">
            <button type="button" onClick={copyConnectionDetails} disabled={!connectionPayload}>
              Copy connection details
            </button>
            <span>{formatCopyState(copyState)}</span>
          </div>
        </section>

        <section className="panel" aria-labelledby="devices-title">
          <div className="panel-heading">
            <div>
              <p className="section-label">Connections</p>
              <h2 id="devices-title">Connected devices</h2>
            </div>
            <button type="button" onClick={disconnectClients} disabled={!serverStatus?.connectedClientCount}>
              Disconnect all
            </button>
          </div>
          <DeviceList clients={serverStatus?.connectedClients ?? []} />
        </section>

        <section className="panel" aria-labelledby="paired-title">
          <div className="panel-heading">
            <div>
              <p className="section-label">Pairings</p>
              <h2 id="paired-title">Saved devices</h2>
            </div>
          </div>
          <PairedDeviceList devices={pairedDevices} />
        </section>

        <section className="panel details-panel" aria-labelledby="details-title">
          <p className="section-label">Server</p>
          <h2 id="details-title">Status</h2>
          <div className="meta-grid">
            <MetaItem label="State" value={serverStatus?.state ?? 'Loading.'} />
            <MetaItem label="Port" value={serverStatus ? String(serverStatus.port) : 'Loading.'} />
            <MetaItem label="Last command" value={formatTimestamp(serverStatus?.lastSeenAt ?? null)} />
            <MetaItem label="Recent error" value={serverStatus?.lastError ?? 'No recent errors.'} />
          </div>
        </section>
      </section>
    </main>
  );
}

function MetaItem({ label, value }: { label: string; value: string }): ReactElement {
  return (
    <div className="meta-item">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function PairedDeviceList({ devices }: { devices: PairedDeviceView[] }): ReactElement {
  if (devices.length === 0) {
    return <div className="empty-state">No devices paired.</div>;
  }

  return (
    <ul className="device-list">
      {devices.map((device) => (
        <li key={device.deviceId}>
          <div>
            <strong>{device.deviceName}</strong>
            <span>{device.deviceId}</span>
          </div>
          <div className="device-times">
            <span>Paired {formatTimestamp(device.pairedAt)}</span>
            <span>Last seen {formatTimestamp(device.lastSeenAt)}</span>
          </div>
        </li>
      ))}
    </ul>
  );
}

function DeviceList({ clients }: { clients: PcConnectedClient[] }): ReactElement {
  if (clients.length === 0) {
    return <div className="empty-state">No devices connected.</div>;
  }

  return (
    <ul className="device-list">
      {clients.map((client) => (
        <li key={client.id}>
          <div>
            <strong>{client.deviceId ?? 'Unidentified device'}</strong>
            <span>{client.remoteAddress ?? 'Unknown address'}</span>
          </div>
          <time>{formatTimestamp(client.lastSeenAt ?? client.connectedAt)}</time>
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

function formatServerStatus(status: PcServerStatus | null): string {
  if (!status) return 'Loading...';
  if (status.state === 'listening') return `Listening on port ${status.port}`;
  if (status.state === 'error') return 'Server error';
  return `${status.state.charAt(0).toUpperCase()}${status.state.slice(1)}...`;
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
