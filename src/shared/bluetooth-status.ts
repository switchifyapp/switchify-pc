export type BluetoothTransportStatus =
  | 'disabled'
  | 'starting'
  | 'ready'
  | 'unavailable'
  | 'connected'
  | 'error'
  | 'stopped';

export type BluetoothUnavailableReason = 'unsupported' | 'permission_denied' | 'adapter_off' | 'startup_failed';

export type BluetoothDisconnectReason =
  | 'notification_unsubscribed'
  | 'pc_requested'
  | 'helper_stopped'
  | 'helper_error'
  | null;

export type BluetoothDiagnosticEvent =
  | 'advertising_started'
  | 'subscribed'
  | 'unsubscribed'
  | 'unsubscribe_grace_started'
  | 'unsubscribe_grace_cancelled'
  | 'unsubscribe_grace_timed_out'
  | 'write_received'
  | null;

export type BluetoothDiagnosticRecord = {
  event: Exclude<BluetoothDiagnosticEvent, null>;
  at: number;
};

export type BluetoothStatus = {
  status: BluetoothTransportStatus;
  reason: BluetoothUnavailableReason | null;
  connectedClientCount: number;
  lastError: string | null;
  lastEvent: BluetoothDiagnosticEvent;
  lastEventAt: number | null;
  recentEvents: BluetoothDiagnosticRecord[];
  lastDisconnectReason: BluetoothDisconnectReason;
  lastDisconnectAt: number | null;
};

export const DEFAULT_BLUETOOTH_STATUS: BluetoothStatus = {
  status: 'disabled',
  reason: null,
  connectedClientCount: 0,
  lastError: null,
  lastEvent: null,
  lastEventAt: null,
  recentEvents: [],
  lastDisconnectReason: null,
  lastDisconnectAt: null
};

