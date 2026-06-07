export type PairingApprovalDecision = 'accept' | 'reject';

export type PendingPairingApprovalView = {
  requestId: string;
  deviceName: string;
  verificationCode: string;
  requestedAt: number;
  expiresAt: number;
  remoteAddress: string | null;
};
