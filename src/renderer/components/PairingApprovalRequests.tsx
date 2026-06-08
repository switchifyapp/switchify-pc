import type { ReactElement } from 'react';
import type { PairingApprovalDecision, PendingPairingApprovalView } from '../../shared/pairing-approval';

export function PairingApprovalRequests({
  requests,
  onRespond
}: {
  requests: PendingPairingApprovalView[];
  onRespond: (requestId: string, decision: PairingApprovalDecision) => Promise<void>;
}): ReactElement | null {
  if (requests.length === 0) return null;

  const sortedRequests = [...requests].sort((a, b) => b.requestedAt - a.requestedAt);

  return (
    <section className="approval-panel" aria-label="Device connection requests">
      <div className="approval-panel-header">
        <div>
          <h2>{sortedRequests[0].deviceName} wants to connect</h2>
          <p>Confirm this code matches your device.</p>
          <div className="approval-code" aria-label="Verification code">
            {sortedRequests[0].verificationCode}
          </div>
        </div>
        <div className="approval-actions">
          <button
            type="button"
            className="primary-button"
            onClick={() => void onRespond(sortedRequests[0].requestId, 'accept')}
          >
            Accept
          </button>
          <button type="button" onClick={() => void onRespond(sortedRequests[0].requestId, 'reject')}>
            Not now
          </button>
        </div>
      </div>
      {sortedRequests.length > 1 ? (
        <ul className="approval-list">
          {sortedRequests.slice(1).map((request) => (
            <li key={request.requestId}>
              <span>{request.deviceName}</span>
              <strong>{request.verificationCode}</strong>
              <button type="button" onClick={() => void onRespond(request.requestId, 'accept')}>
                Accept
              </button>
              <button type="button" onClick={() => void onRespond(request.requestId, 'reject')}>
                Not now
              </button>
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  );
}
