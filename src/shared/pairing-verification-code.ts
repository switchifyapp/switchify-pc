export const PAIRING_VERIFICATION_CODE_LENGTH = 6;

export function createPairingVerificationCode(input: {
  desktopId: string;
  deviceId: string;
  requestNonce: string;
}): string {
  const canonical = `${input.desktopId}\n${input.deviceId}\n${input.requestNonce}`;
  let hash = 2166136261;
  for (const char of canonical) {
    hash ^= char.charCodeAt(0);
    hash = Math.imul(hash, 16777619);
  }
  const value = Math.abs(hash) % 1_000_000;
  return value.toString().padStart(PAIRING_VERIFICATION_CODE_LENGTH, '0');
}
