import type { ReactElement } from 'react';
import type { PendingPairingApprovalView } from '../../shared/pairing-approval';
import type {
  ConnectionDetails,
  PairedDeviceView,
  PairingSessionView,
  PcConnectedClient,
  PcServerStatus
} from '../../shared/server-status';
import { formatCopyState, formatExpiry, formatTimestamp, type CopyState } from '../format';
import { DetailGrid, DetailItem, TroubleshootingSection } from './DetailGrid';

export function TroubleshootingDetails({
  serverStatus,
  connectionDetails,
  pairingSession,
  pairedDevices,
  connectedClients,
  pendingPairingRequests,
  connectionPayload,
  copyState,
  cursorOverlayEnabled,
  onCopy,
  onDisconnect,
  onToggleCursorOverlay
}: {
  serverStatus: PcServerStatus | null;
  connectionDetails: ConnectionDetails | null;
  pairingSession: PairingSessionView | null;
  pairedDevices: PairedDeviceView[];
  connectedClients: PcConnectedClient[];
  pendingPairingRequests: PendingPairingApprovalView[];
  connectionPayload: string;
  copyState: CopyState;
  cursorOverlayEnabled: boolean;
  onCopy: () => Promise<void>;
  onDisconnect: () => Promise<void>;
  onToggleCursorOverlay: (enabled: boolean) => Promise<void>;
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
        <TroubleshootingSection title="Pending requests">
          <PendingRequestList requests={pendingPairingRequests} />
        </TroubleshootingSection>
        <TroubleshootingSection title="Cursor highlight">
          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={cursorOverlayEnabled}
              onChange={(event) => void onToggleCursorOverlay(event.currentTarget.checked)}
            />
            <span>Show cursor highlight when the phone moves the mouse</span>
          </label>
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

function PendingRequestList({ requests }: { requests: PendingPairingApprovalView[] }): ReactElement {
  if (requests.length === 0) {
    return <div className="empty-state">No pending requests.</div>;
  }

  return (
    <ul className="technical-list">
      {requests.map((request) => (
        <li key={request.requestId}>
          <strong>{request.deviceName}</strong>
          <span>{request.remoteAddress ?? 'Unknown address'}</span>
          <span>Requested {formatTimestamp(request.requestedAt)}</span>
          <span>Expires {formatTimestamp(request.expiresAt)}</span>
        </li>
      ))}
    </ul>
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

function isLoopbackUrl(url: string): boolean {
  return url.includes('://127.0.0.1:') || url.includes('://localhost:');
}
