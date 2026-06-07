import { randomBytes } from 'node:crypto';
import type { PairingStore } from './pairing-store';

export const TOKEN_BYTE_LENGTH = 32;

export class PairingManager {
  constructor(private readonly store: PairingStore) {}

  async getDesktopId(): Promise<string> {
    return (await this.store.load()).desktopId;
  }
}

export function createToken(byteLength: number = TOKEN_BYTE_LENGTH): string {
  return randomBytes(byteLength).toString('base64url');
}
