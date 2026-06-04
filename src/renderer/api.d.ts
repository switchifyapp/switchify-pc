export {};

import type { ConnectionDetails, PairingSessionView, PcServerStatus } from '../shared/server-status';

declare global {
  interface Window {
    switchifyPc: {
      appName: string;
      getServerStatus: () => Promise<PcServerStatus>;
      getPairingSession: () => Promise<PairingSessionView | null>;
      createPairingSession: () => Promise<PairingSessionView>;
      getConnectionDetails: () => Promise<ConnectionDetails>;
      disconnectClients: () => Promise<PcServerStatus>;
    };
  }
}
