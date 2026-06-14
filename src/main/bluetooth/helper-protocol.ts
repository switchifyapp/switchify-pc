import type { BluetoothFrame } from '../../shared/bluetooth-frame';
import type {
  BluetoothDiagnosticEvent,
  BluetoothDisconnectReason,
  BluetoothUnavailableReason
} from '../../shared/bluetooth-status';

export type BluetoothHelperEvent =
  | { type: 'ready' }
  | { type: 'unavailable'; reason: BluetoothUnavailableReason }
  | { type: 'connected'; connectionId: string; label: string }
  | { type: 'message'; connectionId: string; frame: BluetoothFrame }
  | { type: 'disconnected'; connectionId: string; reason: Exclude<BluetoothDisconnectReason, null> }
  | { type: 'diagnostic'; event: Exclude<BluetoothDiagnosticEvent, null> }
  | { type: 'error'; reason: string };

export type BluetoothHelperCommand =
  | {
      type: 'start';
      serviceUuid: string;
      rxCharacteristicUuid: string;
      txCharacteristicUuid: string;
      statusCharacteristicUuid: string;
      displayName: string;
      desktopId: string;
    }
  | { type: 'stop' }
  | { type: 'send'; connectionId: string; frame: BluetoothFrame }
  | { type: 'disconnect'; connectionId: string }
  | { type: 'shutdown' };

