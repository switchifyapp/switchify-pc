import type { ReactElement } from 'react';
import type { BluetoothStatus } from '../../shared/bluetooth-status';
import type { DesktopUiState } from '../../shared/desktop-ui-state';
import { bluetoothPrimaryCopy } from '../bluetooth-primary';
import type { ConnectedDeviceView } from '../connected-devices';

export function PrimaryContent({
  state,
  bluetoothStatus,
  connectedDevices,
  onDisconnect,
  onRefresh
}: {
  state: DesktopUiState;
  bluetoothStatus: BluetoothStatus | null;
  connectedDevices: ConnectedDeviceView[];
  onDisconnect: () => Promise<void>;
  onRefresh: () => Promise<void>;
}): ReactElement {
  if (state === 'connected') {
    return (
      <ConnectedReadyState
        connectedDevices={connectedDevices}
        onDisconnect={onDisconnect}
      />
    );
  }

  return <BluetoothState state={state} bluetoothStatus={bluetoothStatus} onRefresh={onRefresh} />;
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
      <p>You can control this PC from Switchify over Bluetooth.</p>
      <DeviceSummary devices={connectedDevices} />
      <div className="action-row centered">
        <button type="button" className="primary-button" onClick={() => void onDisconnect()}>
          Disconnect device
        </button>
      </div>
    </section>
  );
}

function BluetoothState({
  state,
  bluetoothStatus,
  onRefresh
}: {
  state: DesktopUiState;
  bluetoothStatus: BluetoothStatus | null;
  onRefresh: () => Promise<void>;
}): ReactElement {
  const copy = bluetoothPrimaryCopy(state, bluetoothStatus);

  return (
    <section className="primary-state">
      <h2>{copy.title}</h2>
      <p>{copy.body}</p>
      <div className={`bluetooth-connection-panel bluetooth-connection-panel-${copy.tone}`}>
        <div className="bluetooth-status-row">
          <span className="bluetooth-status-indicator" aria-hidden="true" />
          <span>{bluetoothStatusLabel(bluetoothStatus)}</span>
        </div>
      </div>
      {copy.tone === 'error' ? (
        <div className="action-row centered">
          <button type="button" className="primary-button" onClick={() => void onRefresh()}>
            Refresh
          </button>
        </div>
      ) : null}
    </section>
  );
}

function bluetoothStatusLabel(status: BluetoothStatus | null): string {
  if (!status) return 'Bluetooth status unknown.';
  if (status.status === 'ready') return 'Bluetooth ready.';
  if (status.status === 'connected') return 'Bluetooth device connected.';
  if (status.status === 'starting') return 'Starting Bluetooth...';
  if (status.status === 'unavailable' && status.reason === 'adapter_off') return 'Bluetooth is off.';
  if (status.status === 'unavailable' && status.reason === 'permission_denied') return 'Bluetooth permission denied.';
  if (status.status === 'unavailable') return 'Bluetooth unavailable.';
  if (status.status === 'error') return 'Bluetooth needs attention.';
  return 'Bluetooth stopped.';
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
