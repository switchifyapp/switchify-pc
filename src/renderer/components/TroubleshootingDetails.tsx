import { useState, type ReactElement } from 'react';
import type { FirewallDiagnostics, FirewallRepairResult } from '../../shared/firewall';
import type { PendingPairingApprovalView } from '../../shared/pairing-approval';
import type {
  ConnectionDetails,
  PcServerStatus
} from '../../shared/server-status';
import { formatTimestamp } from '../format';
import { DetailGrid, DetailItem, TroubleshootingSection } from './DetailGrid';

type FirewallRepairFailureReason = Extract<FirewallRepairResult, { ok: false }>['reason'];

export function TroubleshootingDetails({
  serverStatus,
  connectionDetails,
  pendingPairingRequests,
  firewallDiagnostics,
  onRepairFirewall
}: {
  serverStatus: PcServerStatus | null;
  connectionDetails: ConnectionDetails | null;
  pendingPairingRequests: PendingPairingApprovalView[];
  firewallDiagnostics: FirewallDiagnostics | null;
  onRepairFirewall: () => Promise<FirewallRepairResult>;
}): ReactElement {
  return (
    <details className="troubleshooting-panel">
      <summary>Troubleshooting</summary>
      <div className="troubleshooting-content">
        <TroubleshootingSection title="Pending requests">
          <PendingRequestList requests={pendingPairingRequests} />
        </TroubleshootingSection>
        <FirewallSection
          diagnostics={firewallDiagnostics}
          onRepairFirewall={onRepairFirewall}
        />
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

function FirewallSection({
  diagnostics,
  onRepairFirewall
}: {
  diagnostics: FirewallDiagnostics | null;
  onRepairFirewall: () => Promise<FirewallRepairResult>;
}): ReactElement {
  const [repairing, setRepairing] = useState(false);
  const [repairError, setRepairError] = useState<string | null>(null);
  const tcpRule = diagnostics?.rules.find((rule) => rule.protocol === 'TCP');
  const mdnsRule = diagnostics?.rules.find((rule) => rule.protocol === 'UDP');
  const profileLabel = diagnostics?.networkProfiles.length
    ? diagnostics.networkProfiles.map((profile) => `${profile.name}: ${profile.category}`).join(', ')
    : 'Unknown';
  const hasPublicProfile = diagnostics?.networkProfiles.some((profile) => profile.category === 'Public') ?? false;
  const statusLabel = firewallStatusLabel(diagnostics);

  const handleRepair = async (): Promise<void> => {
    setRepairing(true);
    setRepairError(null);
    try {
      const result = await onRepairFirewall();
      if (!result.ok) {
        setRepairError(firewallRepairError(result.reason));
      }
    } catch {
      setRepairError('Could not update Windows Firewall.');
    } finally {
      setRepairing(false);
    }
  };

  return (
    <TroubleshootingSection title="Firewall">
      <DetailGrid>
        <DetailItem label="Firewall rules" value={statusLabel} />
        <DetailItem label="TCP 7347" value={tcpRule?.present ? 'Present' : 'Missing'} />
        <DetailItem label="mDNS UDP 5353" value={mdnsRule?.present ? 'Present' : 'Missing'} />
        <DetailItem label="Network profile" value={profileLabel} />
      </DetailGrid>
      {hasPublicProfile ? (
        <p className="inline-warning">Public network detected. Firewall rules must allow Public networks.</p>
      ) : null}
      {repairError ? <p className="inline-error">{repairError}</p> : null}
      {diagnostics?.needsRepair ? (
        <div className="troubleshooting-action-row">
          <button type="button" className="primary-button" disabled={repairing} onClick={() => void handleRepair()}>
            {repairing ? 'Fixing firewall...' : 'Fix firewall'}
          </button>
        </div>
      ) : null}
    </TroubleshootingSection>
  );
}

function firewallStatusLabel(diagnostics: FirewallDiagnostics | null): string {
  if (!diagnostics) return 'Unknown';
  if (!diagnostics.supported) return 'Unsupported';
  return diagnostics.needsRepair ? 'Needs repair' : 'Ready';
}

function firewallRepairError(reason: FirewallRepairFailureReason): string {
  if (reason === 'elevation_cancelled') return 'Firewall repair was cancelled.';
  if (reason === 'unsupported_platform') return 'Firewall diagnostics are not available on this platform.';
  return 'Could not update Windows Firewall.';
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
