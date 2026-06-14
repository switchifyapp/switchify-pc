import type { BluetoothStatus } from './bluetooth-status';

export type TransportKind = 'bluetooth';

export type PcConnectedClient = {
  id: string;
  deviceId: string | null;
  remoteAddress: string | null;
  connectedAt: number;
  lastSeenAt: number | null;
  transport: TransportKind;
};

export type PcControlStatus = {
  state: 'stopped' | 'starting' | 'ready' | 'error';
  desktopId: string | null;
  connectedClientCount: number;
  connectedClients: PcConnectedClient[];
  lastSeenAt: number | null;
  lastError: string | null;
  bluetooth: BluetoothStatus;
};

export type PcControlStatusListener = (status: PcControlStatus) => void;

export type PairedDeviceView = {
  deviceId: string;
  deviceName: string;
  pairedAt: number;
  lastSeenAt: number | null;
};
