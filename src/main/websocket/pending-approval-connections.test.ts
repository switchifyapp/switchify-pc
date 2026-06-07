import type { WebSocket } from 'ws';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { PendingPairingApprovalConnections } from './pending-approval-connections';

const now = 1_724_000_000_000;

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(now);
});

afterEach(() => {
  vi.useRealTimers();
});

describe('PendingPairingApprovalConnections', () => {
  it('stores and retrieves request sockets', () => {
    const connections = new PendingPairingApprovalConnections();
    const client = createClient();

    connections.set('request-1', client, now + 1_000, vi.fn());

    expect(connections.get('request-1')).toBe(client);
  });

  it('replaces an existing request and clears the old timer', () => {
    const connections = new PendingPairingApprovalConnections();
    const oldExpire = vi.fn();
    const newExpire = vi.fn();
    const replacement = createClient();

    connections.set('request-1', createClient(), now + 1_000, oldExpire);
    connections.set('request-1', replacement, now + 2_000, newExpire);
    vi.advanceTimersByTime(1_500);

    expect(oldExpire).not.toHaveBeenCalled();
    expect(newExpire).not.toHaveBeenCalled();
    expect(connections.get('request-1')).toBe(replacement);

    vi.advanceTimersByTime(500);

    expect(newExpire).toHaveBeenCalledTimes(1);
  });

  it('clears a request socket and timer', () => {
    const connections = new PendingPairingApprovalConnections();
    const onExpire = vi.fn();
    connections.set('request-1', createClient(), now + 1_000, onExpire);

    connections.clear('request-1');
    vi.advanceTimersByTime(1_000);

    expect(connections.get('request-1')).toBeNull();
    expect(onExpire).not.toHaveBeenCalled();
  });

  it('clears all request ids for a client', () => {
    const connections = new PendingPairingApprovalConnections();
    const client = createClient();
    const otherClient = createClient();
    connections.set('request-1', client, now + 1_000, vi.fn());
    connections.set('request-2', otherClient, now + 1_000, vi.fn());
    connections.set('request-3', client, now + 1_000, vi.fn());

    const cleared = connections.clearForClient(client);

    expect(cleared).toEqual(['request-1', 'request-3']);
    expect(connections.get('request-1')).toBeNull();
    expect(connections.get('request-2')).toBe(otherClient);
    expect(connections.get('request-3')).toBeNull();
  });

  it('invokes expiry callbacks', () => {
    const connections = new PendingPairingApprovalConnections();
    const onExpire = vi.fn();
    connections.set('request-1', createClient(), now + 1_000, onExpire);

    vi.advanceTimersByTime(1_000);

    expect(onExpire).toHaveBeenCalledTimes(1);
  });

  it('clears all mappings and timers', () => {
    const connections = new PendingPairingApprovalConnections();
    const firstExpire = vi.fn();
    const secondExpire = vi.fn();
    connections.set('request-1', createClient(), now + 1_000, firstExpire);
    connections.set('request-2', createClient(), now + 1_000, secondExpire);

    connections.clearAll();
    vi.advanceTimersByTime(1_000);

    expect(connections.get('request-1')).toBeNull();
    expect(connections.get('request-2')).toBeNull();
    expect(firstExpire).not.toHaveBeenCalled();
    expect(secondExpire).not.toHaveBeenCalled();
  });
});

function createClient(): WebSocket {
  return {} as WebSocket;
}
