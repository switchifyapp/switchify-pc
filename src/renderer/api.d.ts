export {};

import type { ConnectionDetails, PairedDeviceView, PairingSessionView, PcServerStatus } from '../shared/server-status';

declare global {
  interface Window {
    switchifyPc: {
      appName: string;
      getServerStatus: () => Promise<PcServerStatus>;
      getPairingSession: () => Promise<PairingSessionView | null>;
      createPairingSession: () => Promise<PairingSessionView>;
      getConnectionDetails: () => Promise<ConnectionDetails>;
      getPairedDevices: () => Promise<PairedDeviceView[]>;
      disconnectClients: () => Promise<PcServerStatus>;
    };
  }
}
