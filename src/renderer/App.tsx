import type { ReactElement } from 'react';
import { PairingApprovalRequests } from './components/PairingApprovalRequests';
import { PrimaryContent } from './components/PrimaryContent';
import { StatusHeader } from './components/StatusHeader';
import { TroubleshootingDetails } from './components/TroubleshootingDetails';
import { WindowChrome } from './components/WindowTitleBar';
import { SettingsApp } from './SettingsApp';
import { useSwitchifyPcStatus } from './useSwitchifyPcStatus';

export function App(): ReactElement {
  if (window.location.hash === '#/settings') {
    return <SettingsApp />;
  }

  return <MainApp />;
}

function MainApp(): ReactElement {
  const bridge = window.switchifyPc;
  const status = useSwitchifyPcStatus(bridge);

  return (
    <WindowChrome title={bridge.appName} state={status.uiState} className="app-shell">
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

        <TroubleshootingDetails
          serverStatus={status.serverStatus}
          pendingPairingRequests={status.pendingPairingRequests}
        />
      </section>
    </WindowChrome>
  );
}
