import { describe, expect, it } from 'vitest';
import type { PendingPairingApprovalView } from '../../shared/pairing-approval';
import { approvalHeading } from './PairingApprovalRequests';

describe('approvalHeading', () => {
  it('uses the device name when no authenticated device is connected', () => {
    expect(approvalHeading(pendingRequest(), 0)).toBe('Android phone wants to connect');
  });

  it('uses another device copy when an authenticated device is connected', () => {
    expect(approvalHeading(pendingRequest(), 1)).toBe('Another device wants to connect');
  });
});

function pendingRequest(): PendingPairingApprovalView {
  return {
    requestId: 'request-1',
    deviceName: 'Android phone',
    verificationCode: '123456',
    requestedAt: 1_000,
    expiresAt: 2_000,
    remoteAddress: '127.0.0.1'
  };
}
