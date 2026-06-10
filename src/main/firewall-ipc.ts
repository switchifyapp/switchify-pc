import { ipcMain } from 'electron';
import {
  GET_FIREWALL_DIAGNOSTICS_CHANNEL,
  REPAIR_FIREWALL_CHANNEL
} from '../shared/ipc-channels';
import { getFirewallDiagnostics, repairFirewall } from './windows-firewall';

export function registerFirewallIpc(options: { getAppPath: () => string; getPort: () => number }): void {
  ipcMain.handle(GET_FIREWALL_DIAGNOSTICS_CHANNEL, () =>
    getFirewallDiagnostics({ appPath: options.getAppPath(), port: options.getPort() })
  );
  ipcMain.handle(REPAIR_FIREWALL_CHANNEL, () =>
    repairFirewall({ appPath: options.getAppPath(), port: options.getPort() })
  );
}
