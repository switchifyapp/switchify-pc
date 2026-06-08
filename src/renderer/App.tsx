import type { ReactElement } from 'react';
import { PairingApprovalRequests } from './components/PairingApprovalRequests';
import { PrimaryContent } from './components/PrimaryContent';
import { SettingsPanel } from './components/SettingsPanel';
import { StatusHeader } from './components/StatusHeader';
import { TroubleshootingDetails } from './components/TroubleshootingDetails';
import { WindowTitleBar } from './components/WindowTitleBar';
import { useSwitchifyPcStatus } from './useSwitchifyPcStatus';

export function App(): ReactElement {
  const bridge = window.switchifyPc;
  const status = useSwitchifyPcStatus(bridge);

  return (
    <>
      <WindowTitleBar appName={bridge.appName} state={status.uiState} />
      <main className="app-shell">
        <section className="setup-card" aria-label="Switchify PC setup">
          <StatusHeader state={status.uiState} appName={bridge.appName} />

          <PairingApprovalRequests
            requests={status.pendingPairingRequests}
            onRespond={status.respondToPairingRequest}
          />

          <PrimaryContent
            state={status.uiState}
            connectedClients={status.connectedClients}
            onDisconnect={status.disconnectClients}
            onRefresh={status.refresh}
          />

          <SettingsPanel
            connectedClients={status.connectedClients}
            pairedDevices={status.pairedDevices}
            cursorOverlayEnabled={status.cursorOverlayEnabled}
            onDisconnect={status.disconnectClients}
            onToggleCursorOverlay={status.toggleCursorOverlay}
          />

          <TroubleshootingDetails
            serverStatus={status.serverStatus}
            connectionDetails={status.connectionDetails}
            pendingPairingRequests={status.pendingPairingRequests}
          />
        </section>
      </main>
    </>
  );
}
