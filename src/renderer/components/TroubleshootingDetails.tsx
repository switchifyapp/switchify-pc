import type { ReactElement } from 'react';
import type { PendingPairingApprovalView } from '../../shared/pairing-approval';
import type {
  ConnectionDetails,
  PairedDeviceView,
  PcConnectedClient,
  PcServerStatus
} from '../../shared/server-status';
import { formatTimestamp } from '../format';
import { DetailGrid, DetailItem, TroubleshootingSection } from './DetailGrid';

export function TroubleshootingDetails({
  serverStatus,
  connectionDetails,
  pairedDevices,
  connectedClients,
  pendingPairingRequests,
  cursorOverlayEnabled,
  onDisconnect,
  onToggleCursorOverlay
}: {
  serverStatus: PcServerStatus | null;
  connectionDetails: ConnectionDetails | null;
  pairedDevices: PairedDeviceView[];
  connectedClients: PcConnectedClient[];
  pendingPairingRequests: PendingPairingApprovalView[];
  cursorOverlayEnabled: boolean;
  onDisconnect: () => Promise<void>;
  onToggleCursorOverlay: (enabled: boolean) => Promise<void>;
}): ReactElement {
  return (
    <details className="troubleshooting-panel">
      <summary>Troubleshooting</summary>
      <div className="troubleshooting-content">
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
            <DetailItem label="Connection address" value={connectionDetails?.websocketUrl ?? 'Not available.'} />
            <DetailItem label="Desktop id" value={connectionDetails?.desktopId ?? 'Not available.'} />
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
          <span>Code {request.verificationCode}</span>
          <span>Requested {formatTimestamp(request.requestedAt)}</span>
          <span>Expires {formatTimestamp(request.expiresAt)}</span>
        </li>
      ))}
    </ul>
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
