import { randomUUID } from 'node:crypto';
import type { WebSocket } from 'ws';
import type { PcConnectedClient } from '../../shared/server-status';

export class WebSocketClientRegistry {
  private readonly clients = new Map<WebSocket, PcConnectedClient>();

  add(client: WebSocket, remoteAddress: string | null): PcConnectedClient {
    const connectedClient = {
      id: randomUUID(),
      deviceId: null,
      remoteAddress,
      connectedAt: Date.now(),
      lastSeenAt: null
    } satisfies PcConnectedClient;
    this.clients.set(client, connectedClient);
    return { ...connectedClient };
  }

  remove(client: WebSocket): void {
    this.clients.delete(client);
  }

  closeAll(): void {
    for (const client of this.clients.keys()) {
      client.close();
    }
  }

  clear(): void {
    this.clients.clear();
  }

  get(client: WebSocket): PcConnectedClient | null {
    const connectedClient = this.clients.get(client);
    return connectedClient ? { ...connectedClient } : null;
  }

  getRemoteAddress(client: WebSocket): string | null {
    return this.clients.get(client)?.remoteAddress ?? null;
  }

  markSeen(client: WebSocket, deviceId: string, seenAt: number = Date.now()): void {
    const existing = this.clients.get(client);
    if (!existing) return;

    this.clients.set(client, {
      ...existing,
      deviceId,
      lastSeenAt: seenAt
    });
  }

  snapshot(): PcConnectedClient[] {
    return [...this.clients.values()].map((client) => ({ ...client }));
  }

  count(): number {
    return this.clients.size;
  }
}
