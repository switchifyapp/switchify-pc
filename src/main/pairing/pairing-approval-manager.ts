import type { PendingPairingApprovalView } from '../../shared/pairing-approval';
import { createToken, TOKEN_BYTE_LENGTH } from './pairing-manager';
import type { PairingStore } from './pairing-store';
import { upsertPairedDevice } from './pairing-store';

export const PAIRING_APPROVAL_REQUEST_TTL_MS = 2 * 60 * 1000;

export type PendingPairingApproval = {
  requestId: string;
  deviceId: string;
  deviceName: string;
  desktopId: string;
  requestNonce: string;
  requestedAt: number;
  expiresAt: number;
  remoteAddress: string | null;
};

export type CreatePairingApprovalRequestResult = {
  request: PendingPairingApproval;
  replacedRequestId: string | null;
};

export class PairingApprovalManager {
  private readonly pendingRequests = new Map<string, PendingPairingApproval>();

  constructor(
    private readonly pairingStore: PairingStore,
    private readonly now: () => number = Date.now
  ) {}

  createRequest(input: {
    requestId: string;
    deviceId: string;
    deviceName: string;
    desktopId: string;
    requestNonce: string;
    remoteAddress: string | null;
  }): CreatePairingApprovalRequestResult {
    this.expirePendingRequests();

    const replacedRequestId = this.findRequestIdByDeviceId(input.deviceId);
    if (replacedRequestId) {
      this.pendingRequests.delete(replacedRequestId);
    }

    const requestedAt = this.now();
    const request = {
      ...input,
      requestedAt,
      expiresAt: requestedAt + PAIRING_APPROVAL_REQUEST_TTL_MS
    };
    this.pendingRequests.set(request.requestId, request);

    return {
      request: { ...request },
      replacedRequestId
    };
  }

  listPendingRequests(): PendingPairingApproval[] {
    this.expirePendingRequests();
    return [...this.pendingRequests.values()].sort(sortNewestFirst).map((request) => ({ ...request }));
  }

  listPendingRequestViews(): PendingPairingApprovalView[] {
    return this.listPendingRequests().map((request) => ({
      requestId: request.requestId,
      deviceName: request.deviceName,
      requestedAt: request.requestedAt,
      expiresAt: request.expiresAt,
      remoteAddress: request.remoteAddress
    }));
  }

  getRequest(requestId: string): PendingPairingApproval | null {
    this.expirePendingRequests();
    const request = this.pendingRequests.get(requestId);
    return request ? { ...request } : null;
  }

  async accept(
    requestId: string
  ): Promise<{ ok: true; desktopId: string; deviceId: string; token: string } | { ok: false; reason: string }> {
    const request = this.getRequest(requestId);
    if (!request) return { ok: false, reason: 'pairing_request_not_found' };

    const state = await this.pairingStore.load();
    const token = createToken(TOKEN_BYTE_LENGTH);
    await this.pairingStore.save(
      upsertPairedDevice(state, {
        deviceId: request.deviceId,
        deviceName: request.deviceName,
        token,
        pairedAt: this.now(),
        lastSeenAt: null
      })
    );
    this.pendingRequests.delete(requestId);

    return {
      ok: true,
      desktopId: state.desktopId,
      deviceId: request.deviceId,
      token
    };
  }

  reject(requestId: string): { ok: true } | { ok: false; reason: string } {
    this.expirePendingRequests();
    if (!this.pendingRequests.delete(requestId)) {
      return { ok: false, reason: 'pairing_request_not_found' };
    }
    return { ok: true };
  }

  expirePendingRequests(): PendingPairingApproval[] {
    const now = this.now();
    const expired: PendingPairingApproval[] = [];
    for (const [requestId, request] of this.pendingRequests.entries()) {
      if (request.expiresAt <= now) {
        this.pendingRequests.delete(requestId);
        expired.push({ ...request });
      }
    }
    return expired;
  }

  private findRequestIdByDeviceId(deviceId: string): string | null {
    for (const [requestId, request] of this.pendingRequests.entries()) {
      if (request.deviceId === deviceId) {
        return requestId;
      }
    }
    return null;
  }
}

function sortNewestFirst(a: PendingPairingApproval, b: PendingPairingApproval): number {
  return b.requestedAt - a.requestedAt;
}
