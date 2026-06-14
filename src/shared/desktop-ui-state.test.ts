import { describe, expect, it } from 'vitest';
import { DEFAULT_BLUETOOTH_STATUS } from './bluetooth-status';
import { deriveDesktopUiState } from './desktop-ui-state';
import type { PairedDeviceView, PcServerStatus } from './server-status';

describe('deriveDesktopUiState', () => {
  it('returns loading when status is missing', () => {
    expect(deriveDesktopUiState(null, [])).toBe('loading');
  });

  it('returns server-error for error status', () => {
    expect(deriveDesktopUiState(serverStatus({ state: 'error' }), [])).toBe('server-error');
  });

  it('returns starting for starting status', () => {
    expect(deriveDesktopUiState(serverStatus({ state: 'starting' }), [])).toBe('starting');
  });

  it('returns not-running for stopped status', () => {
    expect(deriveDesktopUiState(serverStatus({ state: 'stopped' }), [])).toBe('not-running');
  });

  it('returns connected when at least one client is connected', () => {
    expect(deriveDesktopUiState(serverStatus({ connectedClientCount: 1 }), [])).toBe('connected');
  });

  it('returns waiting-for-device when saved devices exist without active clients', () => {
    expect(deriveDesktopUiState(serverStatus(), [pairedDevice()])).toBe('waiting-for-device');
  });

  it('returns ready-to-pair when listening without connected or saved devices', () => {
    expect(deriveDesktopUiState(serverStatus(), [])).toBe('ready-to-pair');
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
    listeners: [],
    bluetooth: DEFAULT_BLUETOOTH_STATUS,
    ...overrides
  };
}

function pairedDevice(): PairedDeviceView {
  return {
    deviceId: 'device-1',
    deviceName: 'Device',
    pairedAt: 1_000,
    lastSeenAt: null
  };
}
