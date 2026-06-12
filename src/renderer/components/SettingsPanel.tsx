import { useState, type ReactElement } from 'react';
import type { PairedDeviceView } from '../../shared/server-status';
import type { UpdateState } from '../../shared/update';
import type { ConnectedDeviceView } from '../connected-devices';
import { formatTimestamp } from '../format';
import { UpdatesPanel } from './UpdatesPanel';

type SettingsViewProps = {
  connectedDevices: ConnectedDeviceView[];
  pairedDevices: PairedDeviceView[];
  cursorOverlayEnabled: boolean;
  onDisconnect: () => Promise<void>;
  onForgetPairedDevice: (deviceId: string) => Promise<{ ok: boolean; reason?: string }>;
  onToggleCursorOverlay: (enabled: boolean) => Promise<void>;
  updateState: UpdateState | null;
  isCheckingForUpdates: boolean;
  isDownloadingUpdate: boolean;
  onCheckForUpdates: () => Promise<void>;
  onDownloadUpdate: () => Promise<void>;
  onShowDownloadedUpdate: () => Promise<void>;
};

export function SettingsView({
  connectedDevices,
  pairedDevices,
  cursorOverlayEnabled,
  onDisconnect,
  onForgetPairedDevice,
  onToggleCursorOverlay,
  updateState,
  isCheckingForUpdates,
  isDownloadingUpdate,
  onCheckForUpdates,
  onDownloadUpdate,
  onShowDownloadedUpdate
}: SettingsViewProps): ReactElement {
  return (
    <div className="settings-window-content">
      <section className="settings-window-section">
        <h2>Connection</h2>
        <ConnectedDeviceList devices={connectedDevices} />
        <button type="button" onClick={() => void onDisconnect()} disabled={connectedDevices.length === 0}>
          Disconnect device
        </button>
      </section>
      <section className="settings-window-section">
        <h2>Input</h2>
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={cursorOverlayEnabled}
            onChange={(event) => void onToggleCursorOverlay(event.currentTarget.checked)}
          />
          <span>Show cursor highlight when the device moves the mouse</span>
        </label>
      </section>
      <UpdatesPanel
        state={updateState}
        isChecking={isCheckingForUpdates}
        isDownloading={isDownloadingUpdate}
        onCheck={onCheckForUpdates}
        onDownload={onDownloadUpdate}
        onShowDownloaded={onShowDownloadedUpdate}
      />
      <section className="settings-window-section">
        <h2>Saved devices</h2>
        <PairedDeviceList devices={pairedDevices} onForgetPairedDevice={onForgetPairedDevice} />
      </section>
    </div>
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

function PairedDeviceList({
  devices,
  onForgetPairedDevice
}: {
  devices: PairedDeviceView[];
  onForgetPairedDevice: (deviceId: string) => Promise<{ ok: boolean; reason?: string }>;
}): ReactElement {
  const [confirmingDeviceId, setConfirmingDeviceId] = useState<string | null>(null);
  const [forgetError, setForgetError] = useState<string | null>(null);
  const [forgettingDeviceId, setForgettingDeviceId] = useState<string | null>(null);

  if (devices.length === 0) {
    return <div className="empty-state">No devices saved.</div>;
  }

  const confirmForget = async (deviceId: string): Promise<void> => {
    setForgettingDeviceId(deviceId);
    try {
      const result = await onForgetPairedDevice(deviceId);
      if (result.ok) {
        setConfirmingDeviceId(null);
        setForgetError(null);
        return;
      }
      setForgetError(toForgetDeviceError(result.reason));
    } catch {
      setForgetError('Could not forget that saved device.');
    } finally {
      setForgettingDeviceId(null);
    }
  };

  return (
    <>
      {forgetError ? <div className="inline-error">{forgetError}</div> : null}
      <ul className="technical-list">
        {devices.map((device) => {
          const isConfirming = confirmingDeviceId === device.deviceId;
          const isForgetting = forgettingDeviceId === device.deviceId;
          return (
            <li key={device.deviceId}>
              <strong>{device.deviceName}</strong>
              <span>Paired {formatTimestamp(device.pairedAt)}</span>
              <span>Last seen {formatTimestamp(device.lastSeenAt)}</span>
              <div className="technical-list-actions">
                {isConfirming ? (
                  <>
                    <button
                      type="button"
                      className="danger-button"
                      disabled={isForgetting}
                      onClick={() => void confirmForget(device.deviceId)}
                    >
                      Confirm
                    </button>
                    <button
                      type="button"
                      disabled={isForgetting}
                      onClick={() => {
                        setConfirmingDeviceId(null);
                        setForgetError(null);
                      }}
                    >
                      Cancel
                    </button>
                  </>
                ) : (
                  <button
                    type="button"
                    className="danger-button"
                    onClick={() => {
                      setConfirmingDeviceId(device.deviceId);
                      setForgetError(null);
                    }}
                  >
                    Forget
                  </button>
                )}
              </div>
            </li>
          );
        })}
      </ul>
    </>
  );
}

function toForgetDeviceError(reason: string | undefined): string {
  if (reason === 'device_not_found') return 'That saved device is no longer available.';
  if (reason === 'invalid_device_id') return 'That saved device could not be forgotten.';
  return 'Could not forget that saved device.';
}
