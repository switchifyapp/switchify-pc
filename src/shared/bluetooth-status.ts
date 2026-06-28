export type BluetoothTransportStatus =
  | 'disabled'
  | 'starting'
  | 'ready'
  | 'unavailable'
  | 'connected'
  | 'error'
  | 'stopped';

export type BluetoothSystemRadioState = 'on' | 'off' | 'disabled' | 'unknown';

export type BluetoothSystemStatus = {
  adapterPresent: boolean;
  radioState: BluetoothSystemRadioState;
  isLowEnergySupported: boolean | null;
  isPeripheralRoleSupported: boolean | null;
  lastCheckedAt: number | null;
  lastChangedAt: number | null;
};

export type BluetoothUnavailableReason = 'unsupported' | 'permission_denied' | 'adapter_off' | 'startup_failed';

export type BluetoothDisconnectReason =
  | 'notification_unsubscribed'
  | 'client_requested'
  | 'pc_requested'
  | 'helper_stopped'
  | 'helper_error'
  | 'adapter_off'
  | null;

export type BluetoothDiagnosticEvent =
  | 'advertising_started'
  | 'advertising_restarted'
  | 'system_radio_on'
  | 'system_radio_off'
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
  system: BluetoothSystemStatus;
};

export const DEFAULT_BLUETOOTH_SYSTEM_STATUS: BluetoothSystemStatus = {
  adapterPresent: false,
  radioState: 'unknown',
  isLowEnergySupported: null,
  isPeripheralRoleSupported: null,
  lastCheckedAt: null,
  lastChangedAt: null
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
  lastDisconnectAt: null,
  system: DEFAULT_BLUETOOTH_SYSTEM_STATUS
};

