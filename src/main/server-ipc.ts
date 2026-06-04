import { ipcMain } from 'electron';
import {
  CREATE_PAIRING_SESSION_CHANNEL,
  DISCONNECT_CLIENTS_CHANNEL,
  GET_CONNECTION_DETAILS_CHANNEL,
  GET_PAIRING_SESSION_CHANNEL,
  SERVER_STATUS_CHANNEL
} from '../shared/ipc-channels';
import type { ConnectionDetails, PairingSessionView } from '../shared/server-status';
import type { PairingManager } from './pairing/pairing-manager';
import type { PcWebSocketServer } from './websocket/server';

export function registerServerIpc(server: PcWebSocketServer, pairingManager: PairingManager): void {
  ipcMain.handle(SERVER_STATUS_CHANNEL, () => server.getStatus());
  ipcMain.handle(CREATE_PAIRING_SESSION_CHANNEL, async () => toPairingSessionView(await pairingManager.createPairingSession()));
  ipcMain.handle(GET_PAIRING_SESSION_CHANNEL, () => {
    const session = pairingManager.getActivePairingSession();
    return session ? toPairingSessionView(session) : null;
  });
  ipcMain.handle(GET_CONNECTION_DETAILS_CHANNEL, async () => {
    const status = server.getStatus();
    const session = pairingManager.getActivePairingSession();
    return {
      desktopId: await pairingManager.getDesktopId(),
      websocketUrl: `ws://127.0.0.1:${status.port}`,
      pairingCode: session?.pairingCode ?? null,
      pairingNonce: session?.pairingNonce ?? null,
      expiresAt: session?.expiresAt ?? null
    } satisfies ConnectionDetails;
  });
  ipcMain.handle(DISCONNECT_CLIENTS_CHANNEL, () => server.disconnectClients());
}

function toPairingSessionView(session: PairingSessionView): PairingSessionView {
  return { ...session };
}
