import type { ConnectionDetails, PairedDeviceView, PcServerStatus } from './server-status';

export type DesktopUiState =
  | 'loading'
  | 'server-error'
  | 'starting'
  | 'not-running'
  | 'ready-to-pair'
  | 'connected'
  | 'waiting-for-device';

export function deriveDesktopUiState(
  serverStatus: PcServerStatus | null,
  connectionDetails: ConnectionDetails | null,
  pairedDevices: PairedDeviceView[]
): DesktopUiState {
  if (!serverStatus || !connectionDetails) return 'loading';
  if (serverStatus.state === 'error') return 'server-error';
  if (serverStatus.state === 'starting') return 'starting';
  if (serverStatus.state === 'stopped') return 'not-running';
  if (serverStatus.connectedClientCount > 0) return 'connected';
  if (pairedDevices.length > 0) return 'waiting-for-device';
  return 'ready-to-pair';
}
