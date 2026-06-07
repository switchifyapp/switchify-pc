import { describe, expect, it } from 'vitest';
import {
  createPairingVerificationCode,
  PAIRING_VERIFICATION_CODE_LENGTH
} from './pairing-verification-code';

describe('createPairingVerificationCode', () => {
  it('creates a stable six digit code', () => {
    const input = {
      desktopId: 'desktop-1',
      deviceId: 'android-1',
      requestNonce: 'nonce-1'
    };

    expect(createPairingVerificationCode(input)).toBe(createPairingVerificationCode(input));
    expect(createPairingVerificationCode(input)).toMatch(/^\d{6}$/);
    expect(createPairingVerificationCode(input)).toHaveLength(PAIRING_VERIFICATION_CODE_LENGTH);
  });

  it('changes when verification inputs change', () => {
    const base = {
      desktopId: 'desktop-1',
      deviceId: 'android-1',
      requestNonce: 'nonce-1'
    };

    expect(createPairingVerificationCode({ ...base, requestNonce: 'nonce-2' })).not.toBe(
      createPairingVerificationCode(base)
    );
  });
});
