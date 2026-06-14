import { DEFAULT_BLUETOOTH_STATUS } from '../../shared/bluetooth-status';
import { DEFAULT_WS_PORT, type PcControlStatus, type PcServerListenerStatus } from '../../shared/server-status';

export function createInitialControlStatus(port: number = DEFAULT_WS_PORT): PcControlStatus {
  return {
    state: 'stopped',
    port,
    connectedClientCount: 0,
    connectedClients: [],
    lastSeenAt: null,
    lastError: null,
    listeners: [],
    bluetooth: DEFAULT_BLUETOOTH_STATUS
  };
}

export function cloneControlStatus(status: PcControlStatus): PcControlStatus {
  return {
    ...status,
    connectedClients: status.connectedClients.map((client) => ({ ...client })),
    listeners: status.listeners.map((listener) => ({ ...listener })),
    bluetooth: { ...status.bluetooth }
  };
}

export type ControlTransportStatusUpdate = Partial<
  Pick<PcControlStatus, 'state' | 'port' | 'lastError' | 'listeners'>
> & {
  listeners?: PcServerListenerStatus[];
};

