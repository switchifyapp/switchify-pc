import { describe, expect, it } from 'vitest';
import { createPairingQrPayload, PAIRING_QR_KIND, PAIRING_QR_VERSION } from './pairing-qr';
import type { ConnectionDetails } from './server-status';

describe('createPairingQrPayload', () => {
  it('creates a short-lived pairing bootstrap payload without a token', () => {
    const payload = createPairingQrPayload(connectionDetails());

    expect(payload).toEqual({
      kind: PAIRING_QR_KIND,
      version: PAIRING_QR_VERSION,
      desktopId: 'desktop-1',
      websocketUrl: 'ws://192.168.1.180:7347',
      websocketUrls: ['ws://192.168.1.180:7347', 'ws://127.0.0.1:7347'],
      pairingCode: '123456',
      pairingNonce: 'nonce-1',
      expiresAt: 1_000
    });
    expect(JSON.stringify(payload)).not.toContain('token');
  });

  it('returns null while the active pairing session is incomplete', () => {
    expect(createPairingQrPayload({ ...connectionDetails(), pairingCode: null })).toBeNull();
    expect(createPairingQrPayload({ ...connectionDetails(), pairingNonce: null })).toBeNull();
    expect(createPairingQrPayload({ ...connectionDetails(), expiresAt: null })).toBeNull();
    expect(createPairingQrPayload({ ...connectionDetails(), websocketUrls: [] })).toBeNull();
  });
});

function connectionDetails(): ConnectionDetails {
  return {
    desktopId: 'desktop-1',
    websocketUrl: 'ws://192.168.1.180:7347',
    websocketUrls: ['ws://192.168.1.180:7347', 'ws://127.0.0.1:7347'],
    pairingCode: '123456',
    pairingNonce: 'nonce-1',
    expiresAt: 1_000
  };
}
