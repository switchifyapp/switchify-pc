import type { ReactElement } from 'react';
import { PairingApprovalRequests } from './components/PairingApprovalRequests';
import { PrimaryContent } from './components/PrimaryContent';
import { StatusHeader } from './components/StatusHeader';
import { TroubleshootingDetails } from './components/TroubleshootingDetails';
import { useSwitchifyPcStatus } from './useSwitchifyPcStatus';

export function App(): ReactElement {
  const bridge = window.switchifyPc;
  const status = useSwitchifyPcStatus(bridge);

  return (
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
          showPairingWhileConnected={status.showPairingWhileConnected}
          qrCodeUrl={status.qrCodeUrl}
          isRefreshingPairing={status.isRefreshingPairing}
          onRefreshPairing={status.refreshPairingCode}
          onDisconnect={status.disconnectClients}
          onConnectAnotherPhone={status.connectAnotherPhone}
          onRefresh={status.refresh}
        />

        <TroubleshootingDetails
          serverStatus={status.serverStatus}
          connectionDetails={status.connectionDetails}
          pairingSession={status.pairingSession}
          pairedDevices={status.pairedDevices}
          connectedClients={status.connectedClients}
          pendingPairingRequests={status.pendingPairingRequests}
          connectionPayload={status.connectionPayload}
          copyState={status.copyState}
          cursorOverlayEnabled={status.cursorOverlayEnabled}
          onCopy={status.copyConnectionDetails}
          onDisconnect={status.disconnectClients}
          onToggleCursorOverlay={status.toggleCursorOverlay}
        />
      </section>
    </main>
  );
}
