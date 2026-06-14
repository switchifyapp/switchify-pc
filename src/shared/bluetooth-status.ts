export type BluetoothTransportStatus =
  | 'disabled'
  | 'starting'
  | 'ready'
  | 'unavailable'
  | 'connected'
  | 'error'
  | 'stopped';

export type BluetoothUnavailableReason = 'unsupported' | 'permission_denied' | 'adapter_off' | 'startup_failed';

export type BluetoothStatus = {
  status: BluetoothTransportStatus;
  reason: BluetoothUnavailableReason | null;
  connectedClientCount: number;
  lastError: string | null;
};

export const DEFAULT_BLUETOOTH_STATUS: BluetoothStatus = {
  status: 'disabled',
  reason: null,
  connectedClientCount: 0,
  lastError: null
};

