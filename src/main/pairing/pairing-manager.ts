import { randomBytes } from 'node:crypto';
import type { PairingStore } from './pairing-store';
import { upsertPairedDevice } from './pairing-store';

export const PAIRING_SESSION_TTL_MS = 5 * 60 * 1000;
export const PAIRING_CODE_LENGTH = 6;
export const TOKEN_BYTE_LENGTH = 32;

export type PairingSession = {
  desktopId: string;
  pairingCode: string;
  pairingNonce: string;
  expiresAt: number;
};

export type CompletePairingRequest = {
  deviceId: string;
  deviceName: string;
  pairingCode: string;
  pairingNonce: string;
};

export type CompletePairingResult =
  | { ok: true; desktopId: string; deviceId: string; token: string }
  | { ok: false; reason: 'pairing_expired' | 'pairing_mismatch' };

export class PairingManager {
  private activeSession: PairingSession | null = null;

  constructor(
    private readonly store: PairingStore,
    private readonly now: () => number = Date.now
  ) {}

  async getDesktopId(): Promise<string> {
    return (await this.store.load()).desktopId;
  }

  async createPairingSession(ttlMs: number = PAIRING_SESSION_TTL_MS): Promise<PairingSession> {
    const state = await this.store.load();
    const session = {
      desktopId: state.desktopId,
      pairingCode: createPairingCode(),
      pairingNonce: createToken(16),
      expiresAt: this.now() + ttlMs
    };
    this.activeSession = session;
    return session;
  }

  getActivePairingSession(): PairingSession | null {
    if (!this.activeSession || this.activeSession.expiresAt <= this.now()) {
      this.activeSession = null;
    }
    return this.activeSession ? { ...this.activeSession } : null;
  }

  async completePairing(request: CompletePairingRequest): Promise<CompletePairingResult> {
    const session = this.getActivePairingSession();
    if (!session) return { ok: false, reason: 'pairing_expired' };
    if (session.pairingCode !== request.pairingCode || session.pairingNonce !== request.pairingNonce) {
      return { ok: false, reason: 'pairing_mismatch' };
    }

    const state = await this.store.load();
    const token = createToken(TOKEN_BYTE_LENGTH);
    await this.store.save(
      upsertPairedDevice(state, {
        deviceId: request.deviceId,
        deviceName: request.deviceName,
        token,
        pairedAt: this.now(),
        lastSeenAt: null
      })
    );
    this.activeSession = null;

    return {
      ok: true,
      desktopId: state.desktopId,
      deviceId: request.deviceId,
      token
    };
  }
}

export function createToken(byteLength: number = TOKEN_BYTE_LENGTH): string {
  return randomBytes(byteLength).toString('base64url');
}

function createPairingCode(): string {
  const max = 10 ** PAIRING_CODE_LENGTH;
  const value = randomBytes(4).readUInt32BE(0) % max;
  return value.toString().padStart(PAIRING_CODE_LENGTH, '0');
}
