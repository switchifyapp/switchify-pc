export const DEFAULT_WS_PORT = 7347;

export type PcConnectedClient = {
  id: string;
  deviceId: string | null;
  remoteAddress: string | null;
  connectedAt: number;
  lastSeenAt: number | null;
};

export type PcServerStatus = {
  state: 'stopped' | 'starting' | 'listening' | 'error';
  port: number;
  connectedClientCount: number;
  connectedClients: PcConnectedClient[];
  lastSeenAt: number | null;
  lastError: string | null;
};

export type PcServerStatusListener = (status: PcServerStatus) => void;

export type ConnectionDetails = {
  desktopId: string;
  websocketUrl: string;
  websocketUrls: string[];
};

export type PairedDeviceView = {
  deviceId: string;
  deviceName: string;
  pairedAt: number;
  lastSeenAt: number | null;
};
