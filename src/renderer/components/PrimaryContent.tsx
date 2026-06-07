import type { ReactElement } from 'react';
import type { DesktopUiState } from '../../shared/desktop-ui-state';
import type { PcConnectedClient } from '../../shared/server-status';

export function PrimaryContent({
  state,
  connectedClients,
  onDisconnect,
  onRefresh
}: {
  state: DesktopUiState;
  connectedClients: PcConnectedClient[];
  onDisconnect: () => Promise<void>;
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
        onDisconnect={onDisconnect}
      />
    );
  }

  if (state === 'waiting-for-phone') {
    return <WaitingForPhoneState />;
  }

  return <ReadyToConnectState />;
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

function ReadyToConnectState(): ReactElement {
  return (
    <section className="primary-state">
      <h2>Ready to connect</h2>
      <p>Open Switchify on your phone and choose this PC. Approve the request here when it appears.</p>
    </section>
  );
}

function ConnectedReadyState({
  connectedClients,
  onDisconnect
}: {
  connectedClients: PcConnectedClient[];
  onDisconnect: () => Promise<void>;
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
      </div>
    </section>
  );
}

function WaitingForPhoneState(): ReactElement {
  return (
    <section className="primary-state">
      <h2>Waiting for your phone</h2>
      <p>Open Switchify on your phone and choose this PC to reconnect.</p>
    </section>
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
