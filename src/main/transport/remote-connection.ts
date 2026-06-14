export type TransportKind = 'websocket' | 'bluetooth';

export type RemoteConnection = {
  id: string;
  kind: TransportKind;
  label: string;
  remoteAddress: string | null;
  send: (message: string) => void | Promise<void>;
  close: () => void | Promise<void>;
};

