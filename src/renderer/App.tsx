import { useEffect, useState, type ReactElement } from 'react';
import type { UpdateState } from '../shared/update';
import { AndroidDownloadPanel } from './components/AndroidDownloadPanel';
import { PairingApprovalRequests } from './components/PairingApprovalRequests';
import { PrimaryContent } from './components/PrimaryContent';
import { StatusHeader } from './components/StatusHeader';
import { TroubleshootingDetails } from './components/TroubleshootingDetails';
import { WindowChrome } from './components/WindowTitleBar';
import { SettingsApp } from './SettingsApp';
import { useSwitchifyPcStatus } from './useSwitchifyPcStatus';

export function App(): ReactElement {
  if (window.location.hash === '#/settings' || window.location.hash.startsWith('#/settings/')) {
    return <SettingsApp />;
  }

  return <MainApp />;
}

function MainApp(): ReactElement {
  const bridge = window.switchifyPc;
  const status = useSwitchifyPcStatus(bridge);
  const [updateState, setUpdateState] = useState<UpdateState | null>(null);

  useEffect(() => {
    let cancelled = false;

    const refreshUpdateState = (): void => {
      void bridge.getUpdateState().then((state) => {
        if (!cancelled) {
          setUpdateState(state);
        }
      });
    };

    refreshUpdateState();
    const interval = window.setInterval(refreshUpdateState, 30_000);
    window.addEventListener('focus', refreshUpdateState);

    return () => {
      cancelled = true;
      window.clearInterval(interval);
      window.removeEventListener('focus', refreshUpdateState);
    };
  }, [bridge]);

  return (
    <WindowChrome
      title={bridge.appName}
      state={status.uiState}
      className="app-shell"
      updateState={updateState}
      onOpenUpdates={() => bridge.openSettingsWindow('updates')}
    >
      <section className="setup-card" aria-label="Switchify PC setup">
        <StatusHeader
          state={status.uiState}
          appName={bridge.appName}
          onOpenSettings={bridge.openSettingsWindow}
        />

        <PairingApprovalRequests
          requests={status.pendingPairingRequests}
          connectedDeviceCount={status.connectedDevices.length}
          onRespond={status.respondToPairingRequest}
        />

        <PrimaryContent
          state={status.uiState}
          bluetoothStatus={status.serverStatus?.bluetooth ?? null}
          connectedDevices={status.connectedDevices}
          onDisconnect={status.disconnectClients}
          onRefresh={status.refresh}
        />

        {status.uiState === 'connected' ? null : (
          <AndroidDownloadPanel onOpenDownload={bridge.openExternalUrl} />
        )}

        <TroubleshootingDetails
          serverStatus={status.serverStatus}
          pendingPairingRequests={status.pendingPairingRequests}
        />
      </section>
    </WindowChrome>
  );
}
