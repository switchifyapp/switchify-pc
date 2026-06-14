import type { ReactElement } from 'react';
import type { PendingPairingApprovalView } from '../../shared/pairing-approval';
import type { PcControlStatus } from '../../shared/server-status';
import {
  formatBluetoothDiagnosticEvent,
  formatBluetoothDisconnectReason,
  formatBluetoothStatus
} from '../bluetooth-status';
import { formatTimestamp } from '../format';
import { DetailGrid, DetailItem, TroubleshootingSection } from './DetailGrid';

export function TroubleshootingDetails({
  serverStatus,
  pendingPairingRequests
}: {
  serverStatus: PcControlStatus | null;
  pendingPairingRequests: PendingPairingApprovalView[];
}): ReactElement {
  return (
    <details className="troubleshooting-panel">
      <summary>Troubleshooting</summary>
      <div className="troubleshooting-content">
        <TroubleshootingSection title="Pending requests">
          <PendingRequestList requests={pendingPairingRequests} />
        </TroubleshootingSection>
        <TroubleshootingSection title="Bluetooth details">
          <DetailGrid>
            <DetailItem label="Control status" value={serverStatus?.state ?? 'Unknown'} />
            <DetailItem label="Bluetooth" value={formatBluetoothStatus(serverStatus?.bluetooth)} />
            <DetailItem label="Last Bluetooth event" value={formatBluetoothEvent(serverStatus)} />
            <DetailItem label="Recent Bluetooth events" value={formatBluetoothEvents(serverStatus)} />
            <DetailItem label="Last Bluetooth disconnect" value={formatBluetoothDisconnect(serverStatus)} />
            <DetailItem label="Desktop id" value={serverStatus?.desktopId ?? 'Unknown'} />
            <DetailItem label="Last command" value={formatTimestamp(serverStatus?.lastSeenAt ?? null)} />
            <DetailItem label="Recent error" value={serverStatus?.lastError ?? 'No recent errors.'} />
          </DetailGrid>
        </TroubleshootingSection>
      </div>
    </details>
  );
}

function formatBluetoothEvent(serverStatus: PcControlStatus | null): string {
  const bluetooth = serverStatus?.bluetooth;
  if (!bluetooth?.lastEvent) return 'Not recorded.';
  return `${formatBluetoothDiagnosticEvent(bluetooth.lastEvent)} ${formatTimestamp(bluetooth.lastEventAt)}`;
}

function formatBluetoothEvents(serverStatus: PcControlStatus | null): string {
  const events = serverStatus?.bluetooth?.recentEvents ?? [];
  if (events.length === 0) return 'Not recorded.';
  return events.map((event) => `${formatBluetoothDiagnosticEvent(event.event)} ${formatTimestamp(event.at)}`).join(' ');
}

function formatBluetoothDisconnect(serverStatus: PcControlStatus | null): string {
  const bluetooth = serverStatus?.bluetooth;
  if (!bluetooth?.lastDisconnectReason) return 'Not recorded.';
  return `${formatBluetoothDisconnectReason(bluetooth.lastDisconnectReason)} ${formatTimestamp(bluetooth.lastDisconnectAt)}`;
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
