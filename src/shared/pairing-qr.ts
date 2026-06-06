import type { ConnectionDetails } from './server-status';

export const PAIRING_QR_KIND = 'switchify.pc.pairing';
export const PAIRING_QR_VERSION = 1;

export type PairingQrPayload = {
  kind: typeof PAIRING_QR_KIND;
  version: typeof PAIRING_QR_VERSION;
  desktopId: string;
  websocketUrl: string;
  websocketUrls: string[];
  pairingCode: string;
  pairingNonce: string;
  expiresAt: number;
};

export function createPairingQrPayload(details: ConnectionDetails | null): PairingQrPayload | null {
  if (
    !details ||
    !details.desktopId ||
    !details.websocketUrl ||
    details.websocketUrls.length === 0 ||
    !details.pairingCode ||
    !details.pairingNonce ||
    !details.expiresAt
  ) {
    return null;
  }

  return {
    kind: PAIRING_QR_KIND,
    version: PAIRING_QR_VERSION,
    desktopId: details.desktopId,
    websocketUrl: details.websocketUrl,
    websocketUrls: details.websocketUrls,
    pairingCode: details.pairingCode,
    pairingNonce: details.pairingNonce,
    expiresAt: details.expiresAt
  };
}
