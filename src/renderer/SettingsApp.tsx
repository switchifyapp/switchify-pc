import type { ReactElement } from 'react';
import { SettingsView } from './components/SettingsPanel';
import { useSwitchifyPcStatus } from './useSwitchifyPcStatus';

export function SettingsApp(): ReactElement {
  const bridge = window.switchifyPc;
  const status = useSwitchifyPcStatus(bridge);

  return (
    <main className="settings-window-shell">
      <section className="settings-window-header">
        <p className="section-label">Settings</p>
        <h1>Settings</h1>
      </section>

      <SettingsView
        connectedDevices={status.connectedDevices}
        pairedDevices={status.pairedDevices}
        cursorOverlayEnabled={status.cursorOverlayEnabled}
        onDisconnect={status.disconnectClients}
        onToggleCursorOverlay={status.toggleCursorOverlay}
      />
    </main>
  );
}
