import { useCallback, useEffect, useState, type ReactElement } from 'react';
import type { UpdateState } from '../shared/update';
import { SettingsView } from './components/SettingsPanel';
import { WindowChrome } from './components/WindowTitleBar';
import { useSwitchifyPcStatus } from './useSwitchifyPcStatus';

export function SettingsApp(): ReactElement {
  const bridge = window.switchifyPc;
  const status = useSwitchifyPcStatus(bridge);
  const [updateState, setUpdateState] = useState<UpdateState | null>(null);
  const [isCheckingForUpdates, setIsCheckingForUpdates] = useState(false);
  const [isDownloadingUpdate, setIsDownloadingUpdate] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void bridge.getUpdateState().then((state) => {
      if (!cancelled) {
        setUpdateState(state);
      }
    });

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
              percent: null,
              filePath: null
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

  const showDownloadedUpdate = useCallback(async (): Promise<void> => {
    await bridge.showDownloadedUpdate();
  }, [bridge]);

  return (
    <WindowChrome title={bridge.appName} subtitle="Settings" className="settings-window-shell">
      <section className="settings-window-header">
        <p className="section-label">Settings</p>
        <h1>Settings</h1>
      </section>

      <SettingsView
        connectedDevices={status.connectedDevices}
        pairedDevices={status.pairedDevices}
        cursorOverlayEnabled={status.cursorOverlayEnabled}
        onDisconnect={status.disconnectClients}
        onForgetPairedDevice={status.forgetPairedDevice}
        onToggleCursorOverlay={status.toggleCursorOverlay}
        updateState={updateState}
        isCheckingForUpdates={isCheckingForUpdates}
        isDownloadingUpdate={isDownloadingUpdate}
        onCheckForUpdates={checkForUpdates}
        onDownloadUpdate={downloadUpdate}
        onShowDownloadedUpdate={showDownloadedUpdate}
      />
    </WindowChrome>
  );
}
