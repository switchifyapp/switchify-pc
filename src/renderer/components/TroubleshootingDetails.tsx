import type { ReactElement } from 'react';
import type { PendingPairingApprovalView } from '../../shared/pairing-approval';
import type {
  ConnectionDetails,
  PcServerStatus
} from '../../shared/server-status';
import { formatTimestamp } from '../format';
import { DetailGrid, DetailItem, TroubleshootingSection } from './DetailGrid';

export function TroubleshootingDetails({
  serverStatus,
  connectionDetails,
  pendingPairingRequests
}: {
  serverStatus: PcServerStatus | null;
  connectionDetails: ConnectionDetails | null;
  pendingPairingRequests: PendingPairingApprovalView[];
}): ReactElement {
  return (
    <details className="troubleshooting-panel">
      <summary>Troubleshooting</summary>
      <div className="troubleshooting-content">
        <TroubleshootingSection title="Pending requests">
          <PendingRequestList requests={pendingPairingRequests} />
        </TroubleshootingSection>
        <TroubleshootingSection title="Server details">
          <DetailGrid>
            <DetailItem label="Server status" value={serverStatus?.state ?? 'Unknown'} />
            <DetailItem label="Port" value={serverStatus ? String(serverStatus.port) : 'Unknown'} />
            <DetailItem label="Listeners" value={formatListeners(serverStatus)} />
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

function formatListeners(serverStatus: PcServerStatus | null): string {
  if (!serverStatus || serverStatus.listeners.length === 0) return 'Not available.';

  return serverStatus.listeners
    .map((listener) => {
      const host = listener.family === 'IPv6' ? `[${listener.address}]` : listener.address;
      const suffix = listener.state === 'error' ? ` (${listener.error ?? 'error'})` : '';
      return `${listener.family} ${host}:${listener.port}${suffix}`;
    })
    .join(', ');
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
