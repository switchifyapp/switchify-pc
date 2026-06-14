import { createServer, type IncomingMessage, type Server as HttpServer } from 'node:http';
import type { AddressInfo, ListenOptions } from 'node:net';
import { randomUUID } from 'node:crypto';
import { WebSocket, WebSocketServer } from 'ws';
import {
  DEFAULT_WS_PORT,
  type PcServerListenerStatus,
  type PcServerStatus
} from '../../shared/server-status';
import type { PairingApprovalDecision, PendingPairingApprovalView } from '../../shared/pairing-approval';
import type { ControlService } from '../control/control-service';
import type { RemoteConnection } from '../transport/remote-connection';

export type PcWebSocketServerOptions = {
  port?: number;
  controlService: ControlService;
};

type ListenerFamily = 'IPv4' | 'IPv6';

type ListenerTarget = {
  family: ListenerFamily;
  host: string;
  ipv6Only?: boolean;
};

type PcWebSocketListener = {
  family: ListenerFamily;
  host: string;
  httpServer: HttpServer;
  wsServer: WebSocketServer;
  address: AddressInfo | null;
};

const LISTENER_TARGETS: ListenerTarget[] = [
  { family: 'IPv4', host: '0.0.0.0' },
  { family: 'IPv6', host: '::', ipv6Only: true }
];

export class PcWebSocketServer {
  private listeners: PcWebSocketListener[] = [];

  constructor(private readonly options: PcWebSocketServerOptions) {
    this.options.controlService.updateTransportStatus({ port: options.port ?? DEFAULT_WS_PORT });
  }

  getStatus(): PcServerStatus {
    return this.options.controlService.getStatus();
  }

  getAddresses(): PcServerListenerStatus[] {
    return this.getStatus().listeners;
  }

  async start(): Promise<PcServerStatus> {
    if (this.listeners.length > 0) return this.getStatus();

    this.options.controlService.updateTransportStatus({ state: 'starting', lastError: null, listeners: [] });

    const listenerStatuses: PcServerListenerStatus[] = [];
    const errors: string[] = [];
    let port = this.getStatus().port;

    for (const target of LISTENER_TARGETS) {
      try {
        const listener = await this.startListener(target, port);
        this.listeners.push(listener);
        port = listener.address?.port ?? port;
        const status = toListenerStatus(listener);
        listenerStatuses.push(status);
        console.info(
          `Switchify WebSocket listener started. family=${status.family} address=${status.address} port=${status.port}`
        );
      } catch (error) {
        const message = error instanceof Error ? error.message : 'Unknown listener error.';
        const status = {
          family: target.family,
          address: target.host,
          port,
          state: 'error',
          error: message
        } satisfies PcServerListenerStatus;
        listenerStatuses.push(status);
        errors.push(`${target.family} ${target.host}:${port} failed: ${message}`);
        console.error(
          `Switchify WebSocket listener failed. family=${target.family} address=${target.host} port=${port} error=${message}`
        );
      }
    }

    if (this.listeners.length === 0) {
      const lastError = errors.join('; ') || 'No WebSocket listeners started.';
      this.options.controlService.updateTransportStatus({ state: 'error', lastError, listeners: listenerStatuses });
      throw new Error(lastError);
    }

    this.options.controlService.updateTransportStatus({
      state: 'listening',
      port,
      lastError: errors.length > 0 ? errors.join('; ') : null,
      listeners: listenerStatuses
    });

    return this.getStatus();
  }

  async stop(): Promise<PcServerStatus> {
    const listeners = this.listeners;
    if (listeners.length === 0) {
      this.options.controlService.updateTransportStatus({ state: 'stopped', listeners: [] });
      return this.getStatus();
    }

    await this.options.controlService.closeAllConnections();

    this.listeners = [];
    await Promise.all(listeners.map((listener) => closeListener(listener)));

    this.options.controlService.updateTransportStatus({ state: 'stopped', listeners: [] });
    return this.getStatus();
  }

  disconnectClients(): PcServerStatus {
    return this.options.controlService.disconnectClients();
  }

  disconnectDevice(deviceId: string): PcServerStatus {
    return this.options.controlService.disconnectDevice(deviceId);
  }

  getPendingPairingRequests(): PendingPairingApprovalView[] {
    return this.options.controlService.getPendingPairingRequests();
  }

  async respondToPairingRequest(
    requestId: string,
    decision: PairingApprovalDecision
  ): Promise<{ ok: boolean; reason?: string }> {
    return this.options.controlService.respondToPairingRequest(requestId, decision);
  }

  private handleConnection(client: WebSocket, request: IncomingMessage): void {
    const connection = createWebSocketRemoteConnection(client, request.socket.remoteAddress ?? null);
    this.options.controlService.addRemoteConnection(connection);

    client.on('message', (data) => {
      void this.options.controlService.handleRemoteMessage(connection.id, data.toString());
    });
    client.on('close', () => {
      this.options.controlService.removeRemoteConnection(connection.id);
    });
    client.on('error', (error) => {
      this.options.controlService.updateTransportStatus({ lastError: error.message });
    });
  }

  private startListener(target: ListenerTarget, port: number): Promise<PcWebSocketListener> {
    return new Promise((resolve, reject) => {
      const httpServer = createServer();
      const wsServer = new WebSocketServer({ server: httpServer });
      const listenOptions = {
        host: target.host,
        port,
        ...(target.ipv6Only === undefined ? {} : { ipv6Only: target.ipv6Only })
      } satisfies ListenOptions;

      const cleanup = (): void => {
        httpServer.off('listening', onListening);
        httpServer.off('error', onError);
      };
      const onListening = (): void => {
        cleanup();
        wsServer.on('connection', (client, request) => this.handleConnection(client, request));
        const address = httpServer.address();
        resolve({
          family: target.family,
          host: target.host,
          httpServer,
          wsServer,
          address: typeof address === 'object' && address ? address : null
        });
      };
      const onError = (error: Error): void => {
        cleanup();
        wsServer.close();
        httpServer.close();
        reject(error);
      };

      httpServer.once('listening', onListening);
      httpServer.once('error', onError);
      httpServer.listen(listenOptions);
    });
  }

}

function createWebSocketRemoteConnection(client: WebSocket, remoteAddress: string | null): RemoteConnection {
  return {
    id: randomUUID(),
    kind: 'websocket',
    label: remoteAddress ?? 'WebSocket client',
    remoteAddress,
    send: (message) => {
      if (client.readyState === WebSocket.OPEN) {
        client.send(message);
      }
    },
    close: () => {
      client.close();
    }
  };
}

function toListenerStatus(listener: PcWebSocketListener): PcServerListenerStatus {
  return {
    family: listener.family,
    address: listener.address?.address ?? listener.host,
    port: listener.address?.port ?? 0,
    state: 'listening',
    error: null
  };
}

async function closeListener(listener: PcWebSocketListener): Promise<void> {
  await new Promise<void>((resolve) => {
    listener.wsServer.close(() => resolve());
  });
  await new Promise<void>((resolve, reject) => {
    listener.httpServer.close((error) => {
      if (error) reject(error);
      else resolve();
    });
  });
}
