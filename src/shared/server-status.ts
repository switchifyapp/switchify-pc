export const DEFAULT_WS_PORT = 7347;

export type PcServerStatus = {
  state: 'stopped' | 'starting' | 'listening' | 'error';
  port: number;
  connectedClientCount: number;
  lastSeenAt: number | null;
  lastError: string | null;
};

export type PcServerStatusListener = (status: PcServerStatus) => void;
