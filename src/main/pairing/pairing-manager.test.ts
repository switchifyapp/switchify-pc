import { describe, expect, it } from 'vitest';
import { PairingManager, PAIRING_SESSION_TTL_MS } from './pairing-manager';
import { MemoryPairingStore } from './pairing-store';

describe('PairingManager', () => {
  it('creates a persistent desktop id on first load', async () => {
    const store = new MemoryPairingStore();
    const manager = new PairingManager(store);

    const first = await manager.getDesktopId();
    const second = await manager.getDesktopId();

    expect(first).toHaveLength(36);
    expect(second).toBe(first);
  });

  it('creates expiring pairing sessions', async () => {
    let now = 1_000;
    const manager = new PairingManager(new MemoryPairingStore(), () => now);

    const session = await manager.createPairingSession();

    expect(session.pairingCode).toMatch(/^\d{6}$/);
    expect(session.pairingNonce.length).toBeGreaterThan(10);
    expect(session.expiresAt).toBe(1_000 + PAIRING_SESSION_TTL_MS);
    expect(manager.getActivePairingSession()).toEqual(session);

    now = session.expiresAt;
    expect(manager.getActivePairingSession()).toBeNull();
  });

  it('stores paired devices and returns a shared token', async () => {
    const store = new MemoryPairingStore();
    const manager = new PairingManager(store, () => 2_000);
    const session = await manager.createPairingSession();

    const result = await manager.completePairing({
      deviceId: 'android-1',
      deviceName: 'Android phone',
      pairingCode: session.pairingCode,
      pairingNonce: session.pairingNonce
    });

    expect(result).toMatchObject({ ok: true, deviceId: 'android-1' });
    if (!result.ok) throw new Error('Expected pairing success.');
    expect(result.token.length).toBeGreaterThan(20);

    const state = await store.load();
    expect(state.pairedDevices).toHaveLength(1);
    expect(state.pairedDevices[0]).toMatchObject({
      deviceId: 'android-1',
      deviceName: 'Android phone',
      token: result.token,
      pairedAt: 2_000,
      lastSeenAt: null
    });
    expect(manager.getActivePairingSession()).toBeNull();
  });

  it('rejects expired and mismatched pairing attempts', async () => {
    let now = 1_000;
    const manager = new PairingManager(new MemoryPairingStore(), () => now);
    const session = await manager.createPairingSession(100);

    expect(
      await manager.completePairing({
        deviceId: 'android-1',
        deviceName: 'Android phone',
        pairingCode: '000000',
        pairingNonce: session.pairingNonce
      })
    ).toEqual({ ok: false, reason: 'pairing_mismatch' });

    now = 1_101;
    expect(
      await manager.completePairing({
        deviceId: 'android-1',
        deviceName: 'Android phone',
        pairingCode: session.pairingCode,
        pairingNonce: session.pairingNonce
      })
    ).toEqual({ ok: false, reason: 'pairing_expired' });
  });
});
