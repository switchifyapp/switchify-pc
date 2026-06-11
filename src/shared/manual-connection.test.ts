import { describe, expect, it } from 'vitest';
import {
  SWITCHIFY_MANUAL_CONNECTION_TYPE,
  createManualConnectionPayload,
  validateManualConnectionPayload
} from './manual-connection';

describe('manual connection payload', () => {
  it('creates a versioned payload with desktop id and URLs', () => {
    expect(
      createManualConnectionPayload({
        desktopId: 'desktop-1',
        displayName: 'Switchify PC',
        urls: ['ws://[2001:bb6:a61:3700:574c:69d2:25ce:505]:7347', 'ws://192.168.1.180:7347']
      })
    ).toEqual({
      type: SWITCHIFY_MANUAL_CONNECTION_TYPE,
      version: 1,
      desktopId: 'desktop-1',
      displayName: 'Switchify PC',
      urls: ['ws://[2001:bb6:a61:3700:574c:69d2:25ce:505]:7347', 'ws://192.168.1.180:7347']
    });
  });

  it('does not emit legacy manual QR fields rejected by Android', () => {
    const payload = createManualConnectionPayload({
      desktopId: 'desktop-1',
      displayName: 'Switchify PC',
      urls: ['ws://192.168.1.180:7347']
    });

    expect(payload).not.toHaveProperty('kind');
    expect(payload).not.toHaveProperty('protocolVersion');
    expect(payload).not.toHaveProperty('name');
  });

  it('falls back to Switchify PC for blank display names', () => {
    expect(
      createManualConnectionPayload({
        desktopId: 'desktop-1',
        displayName: ' ',
        urls: ['ws://192.168.1.180:7347']
      }).displayName
    ).toBe('Switchify PC');
  });

  it('rejects empty URL lists and non-websocket URLs', () => {
    expect(() =>
      createManualConnectionPayload({
        desktopId: 'desktop-1',
        displayName: 'Switchify PC',
        urls: []
      })
    ).toThrow('Invalid manual connection payload');

    expect(
      validateManualConnectionPayload({
        type: SWITCHIFY_MANUAL_CONNECTION_TYPE,
        version: 1,
        desktopId: 'desktop-1',
        displayName: 'Switchify PC',
        urls: ['https://192.168.1.180:7347']
      })
    ).toBe(false);
  });

  it('rejects empty desktop ids and forbidden secret-bearing keys', () => {
    expect(
      validateManualConnectionPayload({
        type: SWITCHIFY_MANUAL_CONNECTION_TYPE,
        version: 1,
        desktopId: '',
        displayName: 'Switchify PC',
        urls: ['ws://192.168.1.180:7347']
      })
    ).toBe(false);

    expect(
      ['token', 'auth', 'secret', 'nonce'].map((key) =>
        validateManualConnectionPayload({
          type: SWITCHIFY_MANUAL_CONNECTION_TYPE,
          version: 1,
          desktopId: 'desktop-1',
          displayName: 'Switchify PC',
          urls: ['ws://192.168.1.180:7347'],
          [key]: 'hidden'
        })
      )
    ).toEqual([false, false, false, false]);
  });
});
