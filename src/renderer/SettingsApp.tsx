import { useCallback, useEffect, useState, type ReactElement } from 'react';
import type { SystemStartupSettings } from '../shared/system-startup';
import type { UpdateState } from '../shared/update';
import { SettingsView } from './components/SettingsPanel';
import { WindowChrome } from './components/WindowTitleBar';
import { settingsSectionFromHash } from './settings-route';
import { updateInstallMessage } from './updates';
import { useSwitchifyPcStatus } from './useSwitchifyPcStatus';

export function SettingsApp(): ReactElement {
  const bridge = window.switchifyPc;
  const status = useSwitchifyPcStatus(bridge);
  const [updateState, setUpdateState] = useState<UpdateState | null>(null);
  const [systemStartupSettings, setSystemStartupSettings] = useState<SystemStartupSettings | null>(null);
  const [isCheckingForUpdates, setIsCheckingForUpdates] = useState(false);
  const [isDownloadingUpdate, setIsDownloadingUpdate] = useState(false);
  const [isInstallingUpdate, setIsInstallingUpdate] = useState(false);
  const [updateInstallError, setUpdateInstallError] = useState<string | null>(null);
  const [isUpdatingSystemStartup, setIsUpdatingSystemStartup] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void Promise.all([bridge.getUpdateState(), bridge.getSystemStartupSettings()]).then(
      ([updateState, systemStartupSettings]) => {
        if (!cancelled) {
          setUpdateState(updateState);
          setSystemStartupSettings(systemStartupSettings);
        }
      }
    );

    return () => {
      cancelled = true;
    };
  }, [bridge]);

  const checkForUpdates = useCallback(async (): Promise<void> => {
    setIsCheckingForUpdates(true);
    try {
      setUpdateState(await bridge.checkForUpdates());
    } finally {
      setIsCheckingForUpdates(false);
    }
  }, [bridge]);

  const downloadUpdate = useCallback(async (): Promise<void> => {
    setIsDownloadingUpdate(true);
    setUpdateState((current) =>
      current
        ? {
            ...current,
            download: {
              status: 'downloading',
              downloadedBytes: 0,
              totalBytes: null,
              percent: null
            }
          }
        : current
    );
    const progressInterval = window.setInterval(() => {
      void bridge.getUpdateState().then(setUpdateState);
    }, 500);
    try {
      setUpdateState(await bridge.downloadUpdate());
    } finally {
      window.clearInterval(progressInterval);
      setIsDownloadingUpdate(false);
    }
  }, [bridge]);

  const installDownloadedUpdate = useCallback(async (): Promise<void> => {
    setIsInstallingUpdate(true);
    setUpdateInstallError(null);
    try {
      const result = await bridge.installDownloadedUpdate();
      if (!result.ok) {
        setUpdateInstallError(updateInstallMessage(result.reason));
      }
    } finally {
      setIsInstallingUpdate(false);
    }
  }, [bridge]);

  const setStartWithSystem = useCallback(
    async (enabled: boolean): Promise<void> => {
      setIsUpdatingSystemStartup(true);
      try {
        setSystemStartupSettings(await bridge.setStartWithSystem(enabled));
      } finally {
        setIsUpdatingSystemStartup(false);
      }
    },
    [bridge]
  );

  return (
    <WindowChrome
      title={bridge.appName}
      subtitle="Settings"
      className="settings-window-shell"
      windowControls={{ minimize: false }}
    >
      <section className="settings-window-header">
        <p className="section-label">Settings</p>
        <h1>Settings</h1>
      </section>

      <SettingsView
        connectedDevices={status.connectedDevices}
        pairedDevices={status.pairedDevices}
        serverStatus={status.serverStatus}
        cursorOverlaySettings={status.cursorOverlaySettings}
        pointerMovementSettings={status.pointerMovementSettings}
        systemStartupSettings={systemStartupSettings}
        onDisconnect={status.disconnectClients}
        onForgetPairedDevice={status.forgetPairedDevice}
        onUpdateCursorOverlaySettings={status.updateCursorOverlaySettings}
        onUpdatePointerMovementSettings={status.updatePointerMovementSettings}
        isUpdatingSystemStartup={isUpdatingSystemStartup}
        onSetStartWithSystem={setStartWithSystem}
        updateState={updateState}
        isCheckingForUpdates={isCheckingForUpdates}
        isDownloadingUpdate={isDownloadingUpdate}
        isInstallingUpdate={isInstallingUpdate}
        updateInstallError={updateInstallError}
        initialSection={settingsSectionFromHash(window.location.hash)}
        onSettingsSectionRequest={bridge.onShowSettingsSection}
        onCheckForUpdates={checkForUpdates}
        onDownloadUpdate={downloadUpdate}
        onInstallDownloadedUpdate={installDownloadedUpdate}
        />
    </WindowChrome>
  );
}
