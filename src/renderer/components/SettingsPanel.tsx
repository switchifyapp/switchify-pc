import { useEffect, useState, type CSSProperties, type ReactElement } from 'react';
import { MousePointer2 } from 'lucide-react';
import type {
  CursorOverlayColor,
  CursorOverlaySettings,
  CursorOverlaySize,
  CursorOverlayVisibility
} from '../../shared/cursor-overlay-settings';
import { CURSOR_OVERLAY_COLORS } from '../../shared/cursor-overlay-settings';
import {
  BASE_POINTER_MOVEMENT_PERCENTAGES,
  normalizePointerMovementSettings,
  pointerMovementPercentageFor,
  pointerMovementScalePercentFor,
  type PointerMovementSettings,
  type PointerMovementSizeKey
} from '../../shared/pointer-movement-settings';
import type { PairedDeviceView, PcControlStatus } from '../../shared/server-status';
import type { SystemStartupSettings } from '../../shared/system-startup';
import type { SettingsSectionId } from '../../shared/settings';
import type { UpdateState } from '../../shared/update';
import { formatBluetoothStatus } from '../bluetooth-status';
import type { ConnectedDeviceView } from '../connected-devices';
import { formatTimestamp } from '../format';
import { Tooltip } from './Tooltip';
import { UpdatesPanel } from './UpdatesPanel';

type SettingsViewProps = {
  connectedDevices: ConnectedDeviceView[];
  pairedDevices: PairedDeviceView[];
  serverStatus: PcControlStatus | null;
  cursorOverlaySettings: CursorOverlaySettings;
  pointerMovementSettings: PointerMovementSettings;
  systemStartupSettings: SystemStartupSettings | null;
  onDisconnect: () => Promise<void>;
  onForgetPairedDevice: (deviceId: string) => Promise<{ ok: boolean; reason?: string }>;
  onUpdateCursorOverlaySettings: (settings: CursorOverlaySettings) => Promise<void>;
  onUpdatePointerMovementSettings: (settings: PointerMovementSettings) => Promise<void>;
  isUpdatingSystemStartup: boolean;
  onSetStartWithSystem: (enabled: boolean) => Promise<void>;
  updateState: UpdateState | null;
  isCheckingForUpdates: boolean;
  isDownloadingUpdate: boolean;
  initialSection: SettingsSectionId;
  onSettingsSectionRequest?: (handler: (section: SettingsSectionId) => void) => () => void;
  onCheckForUpdates: () => Promise<void>;
  onDownloadUpdate: () => Promise<void>;
  onInstallDownloadedUpdate: () => Promise<void>;
};

const SETTINGS_SECTIONS: Array<{
  id: SettingsSectionId;
  label: string;
}> = [
  { id: 'general', label: 'General' },
  { id: 'bluetooth', label: 'Bluetooth' },
  { id: 'pointer', label: 'Pointer' },
  { id: 'updates', label: 'Updates' },
  { id: 'savedDevices', label: 'Saved devices' }
];

export function SettingsView({
  connectedDevices,
  pairedDevices,
  serverStatus,
  cursorOverlaySettings,
  pointerMovementSettings,
  systemStartupSettings,
  onDisconnect,
  onForgetPairedDevice,
  onUpdateCursorOverlaySettings,
  onUpdatePointerMovementSettings,
  isUpdatingSystemStartup,
  onSetStartWithSystem,
  updateState,
  isCheckingForUpdates,
  isDownloadingUpdate,
  initialSection,
  onSettingsSectionRequest,
  onCheckForUpdates,
  onDownloadUpdate,
  onInstallDownloadedUpdate
}: SettingsViewProps): ReactElement {
  const [selectedSection, setSelectedSection] = useState<SettingsSectionId>(initialSection);

  useEffect(() => {
    if (!onSettingsSectionRequest) return undefined;
    return onSettingsSectionRequest(setSelectedSection);
  }, [onSettingsSectionRequest]);

  return (
    <div className="settings-layout">
      <aside className="settings-side-panel" aria-label="Settings sections">
        {SETTINGS_SECTIONS.map((section) => (
          <button
            key={section.id}
            type="button"
            className={`settings-nav-item${selectedSection === section.id ? ' selected' : ''}`}
            aria-pressed={selectedSection === section.id}
            onClick={() => setSelectedSection(section.id)}
          >
            {section.label}
          </button>
        ))}
      </aside>
      <main className="settings-detail-panel">
        {selectedSection === 'general' ? (
          <GeneralSettingsSection
            systemStartupSettings={systemStartupSettings}
            isUpdatingSystemStartup={isUpdatingSystemStartup}
            onSetStartWithSystem={onSetStartWithSystem}
          />
        ) : null}
        {selectedSection === 'bluetooth' ? (
          <BluetoothSettingsSection
            connectedDevices={connectedDevices}
            serverStatus={serverStatus}
            onDisconnect={onDisconnect}
          />
        ) : null}
        {selectedSection === 'pointer' ? (
          <PointerSettingsSection
            cursorOverlaySettings={cursorOverlaySettings}
            pointerMovementSettings={pointerMovementSettings}
            onUpdateCursorOverlaySettings={onUpdateCursorOverlaySettings}
            onUpdatePointerMovementSettings={onUpdatePointerMovementSettings}
          />
        ) : null}
        {selectedSection === 'updates' ? (
          <UpdatesPanel
            state={updateState}
            isChecking={isCheckingForUpdates}
            isDownloading={isDownloadingUpdate}
            onCheck={onCheckForUpdates}
            onDownload={onDownloadUpdate}
            onInstallDownloaded={onInstallDownloadedUpdate}
          />
        ) : null}
        {selectedSection === 'savedDevices' ? (
          <SavedDevicesSettingsSection
            pairedDevices={pairedDevices}
            onForgetPairedDevice={onForgetPairedDevice}
          />
        ) : null}
      </main>
    </div>
  );
}

function GeneralSettingsSection({
  systemStartupSettings,
  isUpdatingSystemStartup,
  onSetStartWithSystem
}: {
  systemStartupSettings: SystemStartupSettings | null;
  isUpdatingSystemStartup: boolean;
  onSetStartWithSystem: (enabled: boolean) => Promise<void>;
}): ReactElement {
  return (
    <section className="settings-window-section">
      <h2>General</h2>
      <div className="settings-control-group">
        <label className="checkbox-row">
          <input
            type="checkbox"
            checked={Boolean(systemStartupSettings?.startWithSystem)}
            disabled={!systemStartupSettings?.supported || isUpdatingSystemStartup}
            onChange={(event) => void onSetStartWithSystem(event.currentTarget.checked)}
          />
          <span>Start with system</span>
        </label>
        {!systemStartupSettings?.supported ? (
          <div className="empty-state">{systemStartupUnavailableMessage(systemStartupSettings?.reason)}</div>
        ) : null}
      </div>
    </section>
  );
}

function BluetoothSettingsSection({
  connectedDevices,
  serverStatus,
  onDisconnect
}: {
  connectedDevices: ConnectedDeviceView[];
  serverStatus: PcControlStatus | null;
  onDisconnect: () => Promise<void>;
}): ReactElement {
  return (
    <section className="settings-window-section">
      <h2>Bluetooth connection</h2>
      <div className="bluetooth-connection-panel">
        <div className="bluetooth-status-row">
          <span className="bluetooth-status-indicator" aria-hidden="true" />
          <span>{formatBluetoothStatus(serverStatus?.bluetooth)}</span>
        </div>
      </div>
      <ConnectedDeviceList devices={connectedDevices} />
      <button type="button" onClick={() => void onDisconnect()} disabled={connectedDevices.length === 0}>
        Disconnect device
      </button>
    </section>
  );
}

function PointerSettingsSection({
  cursorOverlaySettings,
  pointerMovementSettings,
  onUpdateCursorOverlaySettings,
  onUpdatePointerMovementSettings
}: {
  cursorOverlaySettings: CursorOverlaySettings;
  pointerMovementSettings: PointerMovementSettings;
  onUpdateCursorOverlaySettings: (settings: CursorOverlaySettings) => Promise<void>;
  onUpdatePointerMovementSettings: (settings: PointerMovementSettings) => Promise<void>;
}): ReactElement {
  return (
    <section className="settings-window-section">
      <h2>Pointer</h2>
      <h3>Movement distance</h3>
      <p className="settings-section-note">
        Choose how slow or fast Android pointer steps feel on this display.
      </p>
      <PointerMovementSettingsControls
        settings={pointerMovementSettings}
        onChange={onUpdatePointerMovementSettings}
      />
      <h3>Cursor overlay</h3>
      <CursorOverlaySettingsControls
        settings={cursorOverlaySettings}
        onChange={onUpdateCursorOverlaySettings}
      />
    </section>
  );
}

function PointerMovementSettingsControls({
  settings,
  onChange
}: {
  settings: PointerMovementSettings;
  onChange: (settings: PointerMovementSettings) => Promise<void>;
}): ReactElement {
  const normalizedSettings = normalizePointerMovementSettings(settings);
  const scalePercent = pointerMovementScalePercentFor(normalizedSettings);
  const update = (value: number): void => {
    void onChange(
      normalizePointerMovementSettings({
        scalePercent: value
      })
    );
  };

  return (
    <div className="settings-control-group">
      <div className="pointer-movement-controls">
        <div className="pointer-movement-button-row" role="group" aria-label="Pointer movement scale">
          {pointerMovementScaleOptions.map((value) => (
            <button
              key={value}
              type="button"
              className={value === scalePercent ? 'selected' : undefined}
              aria-pressed={value === scalePercent}
              onClick={() => update(value)}
            >
              {value}%
            </button>
          ))}
        </div>
      </div>
      <PointerMovementPreview settings={normalizedSettings} />
      <PointerMovementTable settings={normalizedSettings} />
    </div>
  );
}

const pointerMovementSizeOptions: Array<{ value: PointerMovementSizeKey; label: string }> = [
  { value: 'small', label: 'Small' },
  { value: 'medium', label: 'Medium' },
  { value: 'large', label: 'Large' }
];

const pointerMovementScaleOptions = [50, 75, 100, 125, 150, 175, 200];

function PointerMovementPreview({ settings }: { settings: PointerMovementSettings }): ReactElement {
  const normalizedSettings = normalizePointerMovementSettings(settings);
  return (
    <div className="pointer-movement-preview" aria-label="Pointer movement preview">
      {pointerMovementSizeOptions.map((option) => {
        const percentage = pointerMovementPercentageFor(normalizedSettings, option.value);
        const distance = Math.min(percentage * 2.4, 88);
        return (
          <div key={option.value} className="pointer-movement-preview-row">
            <span className="pointer-movement-preview-label">{option.label}</span>
            <div className="pointer-movement-preview-track" aria-hidden="true">
              <MousePointer2
                className="pointer-movement-preview-icon"
                style={{ '--pointer-preview-distance': String(distance) } as CSSProperties}
              />
            </div>
            <span className="pointer-movement-preview-value">{percentage}%</span>
          </div>
        );
      })}
    </div>
  );
}

function PointerMovementTable({ settings }: { settings: PointerMovementSettings }): ReactElement {
  const normalizedSettings = normalizePointerMovementSettings(settings);
  return (
    <table className="pointer-movement-table">
      <thead>
        <tr>
          <th scope="col">Step</th>
          <th scope="col">Default</th>
          <th scope="col">Current</th>
        </tr>
      </thead>
      <tbody>
        {pointerMovementSizeOptions.map((option) => (
          <tr key={option.value}>
            <th scope="row">{option.label}</th>
            <td>{BASE_POINTER_MOVEMENT_PERCENTAGES[option.value]}%</td>
            <td>{pointerMovementPercentageFor(normalizedSettings, option.value)}%</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function SavedDevicesSettingsSection({
  pairedDevices,
  onForgetPairedDevice
}: {
  pairedDevices: PairedDeviceView[];
  onForgetPairedDevice: (deviceId: string) => Promise<{ ok: boolean; reason?: string }>;
}): ReactElement {
  return (
    <section className="settings-window-section">
      <h2>Saved devices</h2>
      <PairedDeviceList devices={pairedDevices} onForgetPairedDevice={onForgetPairedDevice} />
    </section>
  );
}

function CursorOverlaySettingsControls({
  settings,
  onChange
}: {
  settings: CursorOverlaySettings;
  onChange: (settings: CursorOverlaySettings) => Promise<void>;
}): ReactElement {
  const update = (update: Partial<CursorOverlaySettings>): void => {
    void onChange({ ...settings, ...update });
  };
  const disabled = !settings.enabled;

  return (
    <div className="settings-control-group">
      <label className="checkbox-row">
        <input
          type="checkbox"
          checked={settings.enabled}
          onChange={(event) => update({ enabled: event.currentTarget.checked })}
        />
        <span>Show cursor overlay</span>
      </label>
      <div className="settings-field">
        <span className="settings-field-label">Size</span>
        <SegmentedControl
          disabled={disabled}
          value={settings.size}
          options={[
            { value: 'small', label: 'Small' },
            { value: 'medium', label: 'Medium' },
            { value: 'large', label: 'Large' }
          ]}
          onChange={(size) => update({ size })}
        />
      </div>
      <div className="settings-field">
        <span className="settings-field-label">Color</span>
        <ColorSwatches
          disabled={disabled}
          value={settings.color}
          onChange={(color) => update({ color })}
        />
      </div>
      <div className="settings-field">
        <span className="settings-field-label">Visibility</span>
        <SegmentedControl
          disabled={disabled}
          value={settings.visibility}
          options={[
            { value: 'onInput', label: 'On input' },
            { value: 'whileControlling', label: 'While controlling' }
          ]}
          onChange={(visibility) => update({ visibility })}
        />
      </div>
      <label className="checkbox-row">
        <input
          type="checkbox"
          checked={settings.crosshairs}
          disabled={disabled}
          onChange={(event) => update({ crosshairs: event.currentTarget.checked })}
        />
        <span>Show full-display crosshairs</span>
      </label>
    </div>
  );
}

function ColorSwatches({
  disabled,
  value,
  onChange
}: {
  disabled: boolean;
  value: CursorOverlayColor;
  onChange: (value: CursorOverlayColor) => void;
}): ReactElement {
  return (
    <div className="color-swatch-row" role="group" aria-label="Cursor overlay color">
      {Object.entries(CURSOR_OVERLAY_COLORS).map(([color, preset]) => (
        <Tooltip key={color} label={preset.label}>
          <button
            type="button"
            className={`color-swatch${value === color ? ' selected' : ''}`}
            style={{ backgroundColor: preset.hex }}
            disabled={disabled}
            aria-label={`Use ${preset.label.toLowerCase()} cursor overlay color`}
            aria-pressed={value === color}
            onClick={() => onChange(color as CursorOverlayColor)}
          />
        </Tooltip>
      ))}
    </div>
  );
}

function SegmentedControl<TValue extends CursorOverlaySize | CursorOverlayVisibility>({
  disabled,
  value,
  options,
  onChange
}: {
  disabled: boolean;
  value: TValue;
  options: Array<{ value: TValue; label: string }>;
  onChange: (value: TValue) => void;
}): ReactElement {
  return (
    <div className="segmented-control" role="group">
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          className={option.value === value ? 'selected' : ''}
          disabled={disabled}
          aria-pressed={option.value === value}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
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
          <span>{formatTransport(device.transport)}</span>
        </li>
      ))}
    </ul>
  );
}

function formatTransport(transport: ConnectedDeviceView['transport']): string {
  if (transport === 'bluetooth') return 'Bluetooth';
  return 'Bluetooth';
}

function systemStartupUnavailableMessage(reason: SystemStartupSettings['reason'] | undefined): string {
  if (reason === 'unpackaged') return 'Start with system is only available in packaged builds.';
  return 'Start with system is not available on this platform.';
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
