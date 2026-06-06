import { describe, expect, it } from 'vitest';
import { deriveDesktopUiState } from './desktop-ui-state';
import type { ConnectionDetails, PairedDeviceView, PcServerStatus } from './server-status';

describe('deriveDesktopUiState', () => {
  it('returns loading when status or connection details are missing', () => {
    expect(deriveDesktopUiState(null, connectionDetails(), [])).toBe('loading');
    expect(deriveDesktopUiState(serverStatus(), null, [])).toBe('loading');
  });

  it('returns server-error for error status', () => {
    expect(deriveDesktopUiState(serverStatus({ state: 'error' }), connectionDetails(), [])).toBe('server-error');
  });

  it('returns starting for starting status', () => {
    expect(deriveDesktopUiState(serverStatus({ state: 'starting' }), connectionDetails(), [])).toBe('starting');
  });

  it('returns not-running for stopped status', () => {
    expect(deriveDesktopUiState(serverStatus({ state: 'stopped' }), connectionDetails(), [])).toBe('not-running');
  });

  it('returns connected when at least one client is connected', () => {
    expect(deriveDesktopUiState(serverStatus({ connectedClientCount: 1 }), connectionDetails(), [])).toBe('connected');
  });

  it('returns waiting-for-phone when saved devices exist without active clients', () => {
    expect(deriveDesktopUiState(serverStatus(), connectionDetails(), [pairedDevice()])).toBe('waiting-for-phone');
  });

  it('returns ready-to-pair when listening without connected or saved devices', () => {
    expect(deriveDesktopUiState(serverStatus(), connectionDetails(), [])).toBe('ready-to-pair');
  });
});

function serverStatus(overrides: Partial<PcServerStatus> = {}): PcServerStatus {
  return {
    state: 'listening',
    port: 7347,
    connectedClientCount: 0,
    connectedClients: [],
    lastSeenAt: null,
    lastError: null,
    ...overrides
  };
}

function connectionDetails(): ConnectionDetails {
  return {
    desktopId: 'desktop-1',
    websocketUrl: 'ws://192.168.1.10:7347',
    websocketUrls: ['ws://192.168.1.10:7347', 'ws://127.0.0.1:7347'],
    pairingCode: '123456',
    pairingNonce: 'nonce',
    expiresAt: Date.now() + 60_000
  };
}

function pairedDevice(): PairedDeviceView {
  return {
    deviceId: 'phone-1',
    deviceName: 'Phone',
    pairedAt: 1_000,
    lastSeenAt: null
  };
}
