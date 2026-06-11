import type { ReactElement } from 'react';
import type { DesktopUiState } from '../../shared/desktop-ui-state';
import type { ConnectionDetails } from '../../shared/server-status';
import type { ConnectedDeviceView } from '../connected-devices';
import { ManualConnectionPanel } from './ManualConnectionPanel';

export function PrimaryContent({
  state,
  appName,
  connectionDetails,
  connectedDevices,
  onDisconnect,
  onRefresh
}: {
  state: DesktopUiState;
  appName: string;
  connectionDetails: ConnectionDetails | null;
  connectedDevices: ConnectedDeviceView[];
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
        connectedDevices={connectedDevices}
        onDisconnect={onDisconnect}
      />
    );
  }

  if (state === 'waiting-for-device') {
    return <WaitingForDeviceState appName={appName} connectionDetails={connectionDetails} />;
  }

  return <ReadyToConnectState appName={appName} connectionDetails={connectionDetails} />;
}

function StartingState({ state }: { state: 'loading' | 'starting' | 'not-running' }): ReactElement {
  if (state === 'not-running') {
    return (
      <section className="primary-state">
        <h2>Switchify PC is not running</h2>
        <p>Restart the app to connect your device.</p>
      </section>
    );
  }

  return (
    <section className="primary-state">
      <h2>{state === 'starting' ? 'Starting Switchify PC...' : 'Getting things ready...'}</h2>
      <p>Switchify PC is preparing your device connection.</p>
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

function ReadyToConnectState({
  appName,
  connectionDetails
}: {
  appName: string;
  connectionDetails: ConnectionDetails | null;
}): ReactElement {
  return (
    <section className="primary-state">
      <h2>Ready to connect</h2>
      <p>Open Switchify on your device and choose this PC. Approve the request here when it appears.</p>
      <ManualConnectionPanel appName={appName} connectionDetails={connectionDetails} />
    </section>
  );
}

function ConnectedReadyState({
  connectedDevices,
  onDisconnect
}: {
  connectedDevices: ConnectedDeviceView[];
  onDisconnect: () => Promise<void>;
}): ReactElement {
  return (
    <section className="primary-state">
      <h2>Your device is connected</h2>
      <p>You can control this PC from Switchify.</p>
      <DeviceSummary devices={connectedDevices} />
      <div className="action-row centered">
        <button type="button" className="primary-button" onClick={() => void onDisconnect()}>
          Disconnect device
        </button>
      </div>
    </section>
  );
}

function WaitingForDeviceState({
  appName,
  connectionDetails
}: {
  appName: string;
  connectionDetails: ConnectionDetails | null;
}): ReactElement {
  return (
    <section className="primary-state">
      <h2>Waiting for your device</h2>
      <p>Open Switchify on your device and choose this PC to reconnect.</p>
      <ManualConnectionPanel appName={appName} connectionDetails={connectionDetails} />
    </section>
  );
}

function DeviceSummary({ devices }: { devices: ConnectedDeviceView[] }): ReactElement {
  const primaryDevice = devices[0];
  return (
    <div className="device-summary">
      <strong>{primaryDevice?.deviceName ?? 'Device connected'}</strong>
      {devices.length > 1 ? <span>{devices.length} devices connected.</span> : <span>Connected now.</span>}
    </div>
  );
}
