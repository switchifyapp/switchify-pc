import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { RemoteConnection } from './remote-connection';
import { RemoteClientRegistry } from './remote-client-registry';

const now = 1_724_000_000_000;

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(now);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('RemoteClientRegistry', () => {
  it('adds connected clients with transport metadata', () => {
    const registry = new RemoteClientRegistry();
    const connection = createConnection();

    const connectedClient = registry.add(connection);

    expect(connectedClient).toEqual({
      id: 'connection-1',
      deviceId: null,
      remoteAddress: null,
      connectedAt: now,
      lastSeenAt: null,
      transport: 'bluetooth'
    });
    expect(registry.count()).toBe(1);
  });

  it('returns authenticated snapshot clones without unauthenticated clients', () => {
    const registry = new RemoteClientRegistry();
    registry.add(createConnection({ id: 'authenticated' }));
    registry.add(createConnection({ id: 'unauthenticated' }));
    registry.markSeen('authenticated', 'android-1');

    const snapshot = registry.authenticatedSnapshot();
    snapshot[0].deviceId = 'mutated';

    expect(snapshot).toHaveLength(1);
    expect(registry.get('authenticated')?.deviceId).toBe('android-1');
    expect(registry.get('unauthenticated')?.deviceId).toBeNull();
  });

  it('closes and removes clients by device id before awaiting transport close', async () => {
    const registry = new RemoteClientRegistry();
    const matching = createConnection({ id: 'matching' });
    const other = createConnection({ id: 'other' });
    registry.add(matching);
    registry.add(other);
    registry.markSeen('matching', 'android-1');
    registry.markSeen('other', 'android-2');

    const closed = await registry.closeByDeviceId('android-1');

    expect(closed).toBe(1);
    expect(matching.close).toHaveBeenCalledTimes(1);
    expect(other.close).not.toHaveBeenCalled();
    expect(registry.get('matching')).toBeNull();
    expect(registry.get('other')?.deviceId).toBe('android-2');
  });
});

function createConnection(overrides: Partial<RemoteConnection> = {}): RemoteConnection {
  return {
    id: 'connection-1',
    kind: 'bluetooth',
    label: 'Test Bluetooth connection',
    remoteAddress: null,
    send: vi.fn(),
    close: vi.fn(),
    ...overrides
  };
}

