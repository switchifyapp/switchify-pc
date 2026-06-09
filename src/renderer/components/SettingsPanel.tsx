import type { ReactElement } from 'react';
import type { PairedDeviceView } from '../../shared/server-status';
import type { ConnectedDeviceView } from '../connected-devices';
import { formatTimestamp } from '../format';
import { TroubleshootingSection } from './DetailGrid';

export function SettingsPanel({
  connectedDevices,
  pairedDevices,
  cursorOverlayEnabled,
  onDisconnect,
  onToggleCursorOverlay
}: {
  connectedDevices: ConnectedDeviceView[];
  pairedDevices: PairedDeviceView[];
  cursorOverlayEnabled: boolean;
  onDisconnect: () => Promise<void>;
  onToggleCursorOverlay: (enabled: boolean) => Promise<void>;
}): ReactElement {
  return (
    <details className="settings-panel">
      <summary>Settings</summary>
      <div className="settings-content">
        <TroubleshootingSection title="Connection">
          <ConnectedDeviceList devices={connectedDevices} />
          <button type="button" onClick={() => void onDisconnect()} disabled={connectedDevices.length === 0}>
            Disconnect device
          </button>
        </TroubleshootingSection>
        <TroubleshootingSection title="Input">
          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={cursorOverlayEnabled}
              onChange={(event) => void onToggleCursorOverlay(event.currentTarget.checked)}
            />
            <span>Show cursor highlight when the device moves the mouse</span>
          </label>
        </TroubleshootingSection>
        <TroubleshootingSection title="Saved devices">
          <PairedDeviceList devices={pairedDevices} />
        </TroubleshootingSection>
      </div>
    </details>
  );
}

function ConnectedDeviceList({ devices }: { devices: ConnectedDeviceView[] }): ReactElement {
  if (devices.length === 0) {
    return <div className="empty-state">No devices connected.</div>;
  }

  return (
    <ul className="technical-list">
      {devices.map((device) => (
        <li key={device.connectionId}>
          <strong>{device.deviceName}</strong>
          <span>{device.remoteAddress ?? 'Unknown address'}</span>
        </li>
      ))}
    </ul>
  );
}

function PairedDeviceList({ devices }: { devices: PairedDeviceView[] }): ReactElement {
  if (devices.length === 0) {
    return <div className="empty-state">No devices saved.</div>;
  }

  return (
    <ul className="technical-list">
      {devices.map((device) => (
        <li key={device.deviceId}>
          <strong>{device.deviceName}</strong>
          <span>Paired {formatTimestamp(device.pairedAt)}</span>
          <span>Last seen {formatTimestamp(device.lastSeenAt)}</span>
        </li>
      ))}
    </ul>
  );
}
