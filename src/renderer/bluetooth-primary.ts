import type { BluetoothStatus } from '../shared/bluetooth-status';
import type { DesktopUiState } from '../shared/desktop-ui-state';

export type BluetoothPrimaryCopy = {
  title: string;
  body: string;
  tone: 'ready' | 'working' | 'error';
};

export function bluetoothPrimaryCopy(state: DesktopUiState, bluetooth: BluetoothStatus | null): BluetoothPrimaryCopy {
  if (state === 'connected') {
    return {
      title: 'Your device is connected',
      body: 'You can control this PC from Switchify over Bluetooth.',
      tone: 'ready'
    };
  }

  if (state === 'waiting-for-device') {
    return {
      title: 'Waiting for your device',
      body: 'Open Switchify near this PC to reconnect over Bluetooth.',
      tone: 'working'
    };
  }

  if (state === 'server-error' || bluetooth?.status === 'error') {
    return {
      title: 'Bluetooth needs attention',
      body: 'Restart Switchify PC. If this keeps happening, open troubleshooting details.',
      tone: 'error'
    };
  }

  if (bluetooth?.status === 'unavailable') {
    if (bluetooth.reason === 'permission_denied') {
      return {
        title: 'Bluetooth permission denied',
        body: 'Allow Bluetooth access for Switchify PC, then restart the app.',
        tone: 'error'
      };
    }

    return {
      title: 'Bluetooth needs attention',
      body: 'Turn on Bluetooth on this PC, then refresh.',
      tone: 'error'
    };
  }

  if (state === 'loading' || state === 'starting' || bluetooth?.status === 'starting') {
    return {
      title: 'Getting Bluetooth ready...',
      body: 'Switchify PC is preparing nearby device connection.',
      tone: 'working'
    };
  }

  if (state === 'not-running' || bluetooth?.status === 'stopped' || bluetooth?.status === 'disabled') {
    return {
      title: 'Switchify PC is not running',
      body: 'Restart the app to connect your device over Bluetooth.',
      tone: 'error'
    };
  }

  return {
    title: 'Ready for Bluetooth',
    body: 'Open Switchify on your Android device while you are near this PC.',
    tone: 'ready'
  };
}

