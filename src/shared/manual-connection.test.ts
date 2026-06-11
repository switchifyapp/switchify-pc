import { describe, expect, it } from 'vitest';
import {
  SWITCHIFY_MANUAL_CONNECTION_KIND,
  createManualConnectionPayload,
  validateManualConnectionPayload
} from './manual-connection';

describe('manual connection payload', () => {
  it('creates a versioned payload with desktop id and URLs', () => {
    expect(
      createManualConnectionPayload({
        desktopId: 'desktop-1',
        name: 'Switchify PC',
        urls: ['ws://[2001:bb6:a61:3700:574c:69d2:25ce:505]:7347', 'ws://192.168.1.180:7347']
      })
    ).toEqual({
      kind: SWITCHIFY_MANUAL_CONNECTION_KIND,
      version: 1,
      protocolVersion: 1,
      desktopId: 'desktop-1',
      name: 'Switchify PC',
      urls: ['ws://[2001:bb6:a61:3700:574c:69d2:25ce:505]:7347', 'ws://192.168.1.180:7347']
    });
  });

  it('rejects empty URL lists and non-websocket URLs', () => {
    expect(() =>
      createManualConnectionPayload({
        desktopId: 'desktop-1',
        name: 'Switchify PC',
        urls: []
      })
    ).toThrow('Invalid manual connection payload');

    expect(
      validateManualConnectionPayload({
        kind: SWITCHIFY_MANUAL_CONNECTION_KIND,
        version: 1,
        protocolVersion: 1,
        desktopId: 'desktop-1',
        name: 'Switchify PC',
        urls: ['https://192.168.1.180:7347']
      })
    ).toBe(false);
  });

  it('rejects empty desktop ids and forbidden secret-bearing keys', () => {
    expect(
      validateManualConnectionPayload({
        kind: SWITCHIFY_MANUAL_CONNECTION_KIND,
        version: 1,
        protocolVersion: 1,
        desktopId: '',
        name: 'Switchify PC',
        urls: ['ws://192.168.1.180:7347']
      })
    ).toBe(false);

    expect(
      validateManualConnectionPayload({
        kind: SWITCHIFY_MANUAL_CONNECTION_KIND,
        version: 1,
        protocolVersion: 1,
        desktopId: 'desktop-1',
        name: 'Switchify PC',
        urls: ['ws://192.168.1.180:7347'],
        token: 'secret'
      })
    ).toBe(false);
  });
});
