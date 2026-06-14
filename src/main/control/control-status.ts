import { DEFAULT_BLUETOOTH_STATUS } from '../../shared/bluetooth-status';
import type { PcControlStatus } from '../../shared/server-status';

export function createInitialControlStatus(): PcControlStatus {
  return {
    state: 'stopped',
    desktopId: null,
    connectedClientCount: 0,
    connectedClients: [],
    lastSeenAt: null,
    lastError: null,
    bluetooth: DEFAULT_BLUETOOTH_STATUS
  };
}

export function cloneControlStatus(status: PcControlStatus): PcControlStatus {
  return {
    ...status,
    connectedClients: status.connectedClients.map((client) => ({ ...client })),
    bluetooth: { ...status.bluetooth }
  };
}


