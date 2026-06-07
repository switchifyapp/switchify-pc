import type { WebSocket } from 'ws';

export class PendingPairingApprovalConnections {
  private readonly clientsByRequestId = new Map<string, WebSocket>();
  private readonly timersByRequestId = new Map<string, ReturnType<typeof setTimeout>>();

  set(requestId: string, client: WebSocket, expiresAt: number, onExpire: () => void): void {
    this.clear(requestId);
    this.clientsByRequestId.set(requestId, client);
    this.timersByRequestId.set(
      requestId,
      setTimeout(() => {
        onExpire();
      }, Math.max(0, expiresAt - Date.now()))
    );
  }

  get(requestId: string): WebSocket | null {
    return this.clientsByRequestId.get(requestId) ?? null;
  }

  clear(requestId: string): void {
    const timer = this.timersByRequestId.get(requestId);
    if (timer) {
      clearTimeout(timer);
    }
    this.timersByRequestId.delete(requestId);
    this.clientsByRequestId.delete(requestId);
  }

  clearForClient(client: WebSocket): string[] {
    const requestIds: string[] = [];
    for (const [requestId, pendingClient] of this.clientsByRequestId.entries()) {
      if (pendingClient === client) {
        requestIds.push(requestId);
      }
    }
    for (const requestId of requestIds) {
      this.clear(requestId);
    }
    return requestIds;
  }

  clearAll(): void {
    for (const timer of this.timersByRequestId.values()) {
      clearTimeout(timer);
    }
    this.timersByRequestId.clear();
    this.clientsByRequestId.clear();
  }
}
