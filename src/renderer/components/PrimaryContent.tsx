import type { ReactElement } from 'react';
import type { DesktopUiState } from '../../shared/desktop-ui-state';
import type { PcConnectedClient } from '../../shared/server-status';

export function PrimaryContent({
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
