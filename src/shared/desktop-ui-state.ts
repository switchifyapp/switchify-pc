import type { PairedDeviceView, PcControlStatus } from './server-status';

export type DesktopUiState =
  | 'loading'
  | 'server-error'
  | 'starting'
  | 'not-running'
  | 'ready-to-pair'
  | 'connected'
  | 'waiting-for-device';

export function deriveDesktopUiState(
  serverStatus: PcControlStatus | null,
  pairedDevices: PairedDeviceView[]
): DesktopUiState {
  if (!serverStatus) return 'loading';
  if (serverStatus.state === 'error') return 'server-error';
  if (serverStatus.state === 'starting') return 'starting';
  if (serverStatus.state === 'stopped') return 'not-running';
  if (serverStatus.connectedClientCount > 0) return 'connected';
  if (pairedDevices.length > 0) return 'waiting-for-device';
  return 'ready-to-pair';
}
