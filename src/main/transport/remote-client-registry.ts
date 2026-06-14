import type { PcConnectedClient } from '../../shared/server-status';
import type { RemoteConnection, TransportKind } from './remote-connection';

export class RemoteClientRegistry {
  private readonly clients = new Map<string, PcConnectedClient>();
  private readonly connections = new Map<string, RemoteConnection>();

  add(connection: RemoteConnection): PcConnectedClient {
    const connectedClient = {
      id: connection.id,
      deviceId: null,
      remoteAddress: connection.remoteAddress,
      connectedAt: Date.now(),
      lastSeenAt: null,
      transport: connection.kind
    } satisfies PcConnectedClient;
    this.connections.set(connection.id, connection);
    this.clients.set(connection.id, connectedClient);
    return { ...connectedClient };
  }

  remove(connectionId: string): void {
    this.connections.delete(connectionId);
    this.clients.delete(connectionId);
  }

  async closeAll(): Promise<void> {
    await Promise.all([...this.connections.values()].map((connection) => connection.close()));
  }

  async closeByDeviceId(deviceId: string): Promise<number> {
    let closedCount = 0;
    const closeResults: Array<void | Promise<void>> = [];
    for (const [connectionId, connectedClient] of this.clients) {
      if (connectedClient.deviceId !== deviceId) continue;

      const connection = this.connections.get(connectionId);
      if (connection) {
        closeResults.push(connection.close());
      }
      this.remove(connectionId);
      closedCount += 1;
    }
    await Promise.all(closeResults);
    return closedCount;
  }

  clear(): void {
    this.connections.clear();
    this.clients.clear();
  }

  get(connectionId: string): PcConnectedClient | null {
    const connectedClient = this.clients.get(connectionId);
    return connectedClient ? { ...connectedClient } : null;
  }

  getConnection(connectionId: string): RemoteConnection | null {
    return this.connections.get(connectionId) ?? null;
  }

  getRemoteAddress(connectionId: string): string | null {
    return this.clients.get(connectionId)?.remoteAddress ?? null;
  }

  getTransport(connectionId: string): TransportKind | null {
    return this.clients.get(connectionId)?.transport ?? null;
  }

  markSeen(connectionId: string, deviceId: string, seenAt: number = Date.now()): void {
    const existing = this.clients.get(connectionId);
    if (!existing) return;

    this.clients.set(connectionId, {
      ...existing,
      deviceId,
      lastSeenAt: seenAt
    });
  }

  snapshot(): PcConnectedClient[] {
    return [...this.clients.values()].map((client) => ({ ...client }));
  }

  authenticatedSnapshot(): PcConnectedClient[] {
    return this.snapshot().filter((client) => client.deviceId !== null);
  }

  count(): number {
    return this.clients.size;
  }

  authenticatedCount(): number {
    let count = 0;
    for (const client of this.clients.values()) {
      if (client.deviceId !== null) count += 1;
    }
    return count;
  }
}
