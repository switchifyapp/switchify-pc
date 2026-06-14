export class PendingPairingApprovalConnections {
  private readonly connectionIdsByRequestId = new Map<string, string>();
  private readonly timersByRequestId = new Map<string, ReturnType<typeof setTimeout>>();

  set(requestId: string, connectionId: string, expiresAt: number, onExpire: () => void): void {
    this.clear(requestId);
    this.connectionIdsByRequestId.set(requestId, connectionId);
    this.timersByRequestId.set(
      requestId,
      setTimeout(() => {
        onExpire();
      }, Math.max(0, expiresAt - Date.now()))
    );
  }

  get(requestId: string): string | null {
    return this.connectionIdsByRequestId.get(requestId) ?? null;
  }

  clear(requestId: string): void {
    const timer = this.timersByRequestId.get(requestId);
    if (timer) {
      clearTimeout(timer);
    }
    this.timersByRequestId.delete(requestId);
    this.connectionIdsByRequestId.delete(requestId);
  }

  clearForConnection(connectionId: string): string[] {
    const requestIds: string[] = [];
    for (const [requestId, pendingConnectionId] of this.connectionIdsByRequestId.entries()) {
      if (pendingConnectionId === connectionId) {
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
    this.connectionIdsByRequestId.clear();
  }
}

