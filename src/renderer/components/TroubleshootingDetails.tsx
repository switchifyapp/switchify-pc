import type { ReactElement } from 'react';
import type { PendingPairingApprovalView } from '../../shared/pairing-approval';
import type { PcServerStatus } from '../../shared/server-status';
import { formatBluetoothStatus } from '../bluetooth-status';
import { formatTimestamp } from '../format';
import { DetailGrid, DetailItem, TroubleshootingSection } from './DetailGrid';

export function TroubleshootingDetails({
  serverStatus,
  pendingPairingRequests
}: {
  serverStatus: PcServerStatus | null;
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
            <DetailItem label="Bluetooth" value={formatBluetoothStatus(serverStatus?.bluetooth)} />
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
          <span>{request.remoteAddress ?? 'Bluetooth'}</span>
          <span>Code {request.verificationCode}</span>
          <span>Requested {formatTimestamp(request.requestedAt)}</span>
          <span>Expires {formatTimestamp(request.expiresAt)}</span>
        </li>
      ))}
    </ul>
  );
}
