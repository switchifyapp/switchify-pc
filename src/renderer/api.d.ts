export {};

import type { PcServerStatus } from '../shared/server-status';

declare global {
  interface Window {
    switchifyPc: {
      appName: string;
      status: string;
      getServerStatus: () => Promise<PcServerStatus>;
    };
  }
}
