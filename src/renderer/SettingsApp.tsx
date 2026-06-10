import type { ReactElement } from 'react';
import { SettingsView } from './components/SettingsPanel';
import { WindowChrome } from './components/WindowTitleBar';
import { useSwitchifyPcStatus } from './useSwitchifyPcStatus';

export function SettingsApp(): ReactElement {
  const bridge = window.switchifyPc;
  const status = useSwitchifyPcStatus(bridge);

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
      />
    </WindowChrome>
  );
}
