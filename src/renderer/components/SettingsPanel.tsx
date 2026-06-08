import type { ReactElement } from 'react';
import type { PairedDeviceView, PcConnectedClient } from '../../shared/server-status';
import { formatTimestamp } from '../format';
import { TroubleshootingSection } from './DetailGrid';

export function SettingsPanel({
  connectedClients,
  pairedDevices,
  cursorOverlayEnabled,
  onDisconnect,
  onToggleCursorOverlay
}: {
  connectedClients: PcConnectedClient[];
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
          <ClientList clients={connectedClients} />
          <button type="button" onClick={() => void onDisconnect()} disabled={connectedClients.length === 0}>
            Disconnect phone
          </button>
        </TroubleshootingSection>
        <TroubleshootingSection title="Input">
          <label className="checkbox-row">
            <input
              type="checkbox"
              checked={cursorOverlayEnabled}
              onChange={(event) => void onToggleCursorOverlay(event.currentTarget.checked)}
            />
            <span>Show cursor highlight when the phone moves the mouse</span>
          </label>
        </TroubleshootingSection>
        <TroubleshootingSection title="Saved phones">
          <PairedDeviceList devices={pairedDevices} />
        </TroubleshootingSection>
      </div>
    </details>
  );
}

function ClientList({ clients }: { clients: PcConnectedClient[] }): ReactElement {
  if (clients.length === 0) {
    return <div className="empty-state">No phones connected.</div>;
  }

  return (
    <ul className="technical-list">
      {clients.map((client) => (
        <li key={client.id}>
          <strong>{client.deviceId ?? 'Unidentified phone'}</strong>
          <span>{client.remoteAddress ?? 'Unknown address'}</span>
        </li>
      ))}
    </ul>
  );
}

function PairedDeviceList({ devices }: { devices: PairedDeviceView[] }): ReactElement {
  if (devices.length === 0) {
    return <div className="empty-state">No phones saved.</div>;
  }

  return (
    <ul className="technical-list">
      {devices.map((device) => (
        <li key={device.deviceId}>
          <strong>{device.deviceName}</strong>
          <span>{device.deviceId}</span>
          <span>Paired {formatTimestamp(device.pairedAt)}</span>
          <span>Last seen {formatTimestamp(device.lastSeenAt)}</span>
        </li>
      ))}
    </ul>
  );
}
