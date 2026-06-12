import type { WebSocket } from 'ws';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { WebSocketClientRegistry } from './client-registry';

const now = 1_724_000_000_000;

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(now);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('WebSocketClientRegistry', () => {
  it('adds a connected client with status metadata', () => {
    const registry = new WebSocketClientRegistry();
    const client = createClient();

    const connectedClient = registry.add(client, '127.0.0.1');

    expect(connectedClient).toMatchObject({
      deviceId: null,
      remoteAddress: '127.0.0.1',
      connectedAt: now,
      lastSeenAt: null
    });
    expect(connectedClient.id).not.toBe('');
    expect(registry.count()).toBe(1);
  });

  it('returns snapshot clones instead of tracked references', () => {
    const registry = new WebSocketClientRegistry();
    const client = createClient();
    registry.add(client, '127.0.0.1');

    const snapshot = registry.snapshot();
    snapshot[0].deviceId = 'mutated';

    expect(registry.snapshot()[0].deviceId).toBeNull();
    expect(registry.get(client)?.deviceId).toBeNull();
  });

  it('returns authenticated snapshot clones without unauthenticated clients', () => {
    const registry = new WebSocketClientRegistry();
    const authenticated = createClient();
    const unauthenticated = createClient();
    registry.add(authenticated, '127.0.0.1');
    registry.add(unauthenticated, '127.0.0.2');
    registry.markSeen(authenticated, 'android-1');

    const snapshot = registry.authenticatedSnapshot();
    snapshot[0].deviceId = 'mutated';

    expect(snapshot).toHaveLength(1);
    expect(snapshot[0]).toMatchObject({ deviceId: 'mutated' });
    expect(registry.get(authenticated)?.deviceId).toBe('android-1');
    expect(registry.get(unauthenticated)?.deviceId).toBeNull();
  });

  it('counts only authenticated clients in authenticatedCount', () => {
    const registry = new WebSocketClientRegistry();
    const authenticated = createClient();
    const unauthenticated = createClient();
    registry.add(authenticated, '127.0.0.1');
    registry.add(unauthenticated, '127.0.0.2');
    registry.markSeen(authenticated, 'android-1');

    expect(registry.count()).toBe(2);
    expect(registry.authenticatedCount()).toBe(1);
  });

  it('marks a tracked client seen', () => {
    const registry = new WebSocketClientRegistry();
    const client = createClient();
    registry.add(client, '127.0.0.1');

    registry.markSeen(client, 'android-1', now + 500);

    expect(registry.get(client)).toMatchObject({
      deviceId: 'android-1',
      lastSeenAt: now + 500
    });
  });

  it('ignores markSeen for untracked clients', () => {
    const registry = new WebSocketClientRegistry();

    registry.markSeen(createClient(), 'android-1', now + 500);

    expect(registry.count()).toBe(0);
  });

  it('removes clients', () => {
    const registry = new WebSocketClientRegistry();
    const client = createClient();
    registry.add(client, '127.0.0.1');

    registry.remove(client);

    expect(registry.count()).toBe(0);
    expect(registry.get(client)).toBeNull();
  });

  it('closes all tracked clients without clearing them', () => {
    const registry = new WebSocketClientRegistry();
    const first = createClient();
    const second = createClient();
    registry.add(first, '127.0.0.1');
    registry.add(second, '127.0.0.2');

    registry.closeAll();

    expect(first.close).toHaveBeenCalledTimes(1);
    expect(second.close).toHaveBeenCalledTimes(1);
    expect(registry.count()).toBe(2);
  });

  it('closes and removes clients by device id', () => {
    const registry = new WebSocketClientRegistry();
    const matching = createClient();
    const otherDevice = createClient();
    const unauthenticated = createClient();
    registry.add(matching, '127.0.0.1');
    registry.add(otherDevice, '127.0.0.2');
    registry.add(unauthenticated, '127.0.0.3');
    registry.markSeen(matching, 'android-1');
    registry.markSeen(otherDevice, 'android-2');

    const closedCount = registry.closeByDeviceId('android-1');

    expect(closedCount).toBe(1);
    expect(matching.close).toHaveBeenCalledTimes(1);
    expect(otherDevice.close).not.toHaveBeenCalled();
    expect(unauthenticated.close).not.toHaveBeenCalled();
    expect(registry.count()).toBe(2);
    expect(registry.get(matching)).toBeNull();
    expect(registry.get(otherDevice)?.deviceId).toBe('android-2');
    expect(registry.get(unauthenticated)?.deviceId).toBeNull();
  });

  it('clears all clients', () => {
    const registry = new WebSocketClientRegistry();
    registry.add(createClient(), '127.0.0.1');
    registry.add(createClient(), '127.0.0.2');

    registry.clear();

    expect(registry.count()).toBe(0);
    expect(registry.snapshot()).toEqual([]);
  });

  it('returns remote addresses for tracked clients', () => {
    const registry = new WebSocketClientRegistry();
    const client = createClient();
    registry.add(client, '127.0.0.1');

    expect(registry.getRemoteAddress(client)).toBe('127.0.0.1');
    expect(registry.getRemoteAddress(createClient())).toBeNull();
  });
});

function createClient(): WebSocket {
  return {
    close: vi.fn()
  } as unknown as WebSocket;
}
