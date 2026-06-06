import { describe, expect, it } from 'vitest';
import { PairingApprovalManager, PAIRING_APPROVAL_REQUEST_TTL_MS } from './pairing-approval-manager';
import { MemoryPairingStore } from './pairing-store';

const now = 1_724_000_000_000;

describe('PairingApprovalManager', () => {
  it('creates pending requests with expiry', () => {
    const manager = createManager();

    const result = manager.createRequest(createRequestInput());

    expect(result.replacedRequestId).toBeNull();
    expect(result.request).toMatchObject({
      requestId: 'approval-1',
      deviceId: 'android-1',
      deviceName: 'Android smoke',
      requestedAt: now,
      expiresAt: now + PAIRING_APPROVAL_REQUEST_TTL_MS
    });
  });

  it('lists renderer-safe pending request views without secrets', () => {
    const manager = createManager();
    manager.createRequest(createRequestInput({ requestNonce: 'secret-nonce' }));

    expect(manager.listPendingRequestViews()).toEqual([
      {
        requestId: 'approval-1',
        deviceName: 'Android smoke',
        requestedAt: now,
        expiresAt: now + PAIRING_APPROVAL_REQUEST_TTL_MS,
        remoteAddress: '192.168.1.50'
      }
    ]);
  });

  it('accepts a request, stores the paired device, and returns a token', async () => {
    const store = createStore();
    const manager = createManager(store);
    manager.createRequest(createRequestInput());

    const result = await manager.accept('approval-1');

    expect(result).toMatchObject({
      ok: true,
      desktopId: 'desktop-1',
      deviceId: 'android-1'
    });
    if (result.ok) {
      expect(result.token).toEqual(expect.any(String));
    }
    expect((await store.load()).pairedDevices[0]).toMatchObject({
      deviceId: 'android-1',
      deviceName: 'Android smoke',
      pairedAt: now,
      lastSeenAt: null
    });
    expect(manager.listPendingRequests()).toHaveLength(0);
  });

  it('rejects a request and removes it', () => {
    const manager = createManager();
    manager.createRequest(createRequestInput());

    expect(manager.reject('approval-1')).toEqual({ ok: true });

    expect(manager.listPendingRequests()).toHaveLength(0);
  });

  it('expires pending requests', () => {
    let currentTime = now;
    const manager = createManager(createStore(), () => currentTime);
    manager.createRequest(createRequestInput());
    currentTime += PAIRING_APPROVAL_REQUEST_TTL_MS + 1;

    const expired = manager.expirePendingRequests();

    expect(expired).toHaveLength(1);
    expect(expired[0].requestId).toBe('approval-1');
    expect(manager.listPendingRequests()).toHaveLength(0);
  });

  it('replaces older pending requests for the same device', () => {
    const manager = createManager();
    manager.createRequest(createRequestInput({ requestId: 'approval-1' }));

    const result = manager.createRequest(createRequestInput({ requestId: 'approval-2' }));

    expect(result.replacedRequestId).toBe('approval-1');
    expect(manager.listPendingRequests()).toHaveLength(1);
    expect(manager.listPendingRequests()[0].requestId).toBe('approval-2');
  });
});

function createManager(store = createStore(), nowFn: () => number = () => now): PairingApprovalManager {
  return new PairingApprovalManager(store, nowFn);
}

function createStore(): MemoryPairingStore {
  return new MemoryPairingStore({
    desktopId: 'desktop-1',
    pairedDevices: []
  });
}

function createRequestInput(overrides: Partial<Parameters<PairingApprovalManager['createRequest']>[0]> = {}) {
  return {
    requestId: 'approval-1',
    deviceId: 'android-1',
    deviceName: 'Android smoke',
    desktopId: 'desktop-1',
    requestNonce: 'nonce',
    remoteAddress: '192.168.1.50',
    ...overrides
  };
}
