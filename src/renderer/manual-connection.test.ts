import { describe, expect, it } from 'vitest';
import { createManualConnectionDetails, nonLoopbackWebSocketUrls } from './manual-connection';

describe('manual connection renderer helpers', () => {
  it('removes loopback and invalid URLs from the QR payload candidates', () => {
    expect(
      nonLoopbackWebSocketUrls([
        'ws://127.0.0.1:7347',
        'ws://[::1]:7347',
        'ws://192.168.1.180:7347',
        'ws://[2001:bb6:a61:3700:574c:69d2:25ce:505]:7347',
        'https://192.168.1.180:7347',
        'not a url'
      ])
    ).toEqual([
      'ws://192.168.1.180:7347',
      'ws://[2001:bb6:a61:3700:574c:69d2:25ce:505]:7347'
    ]);
  });

  it('creates a payload with all non-loopback URL candidates in order', () => {
    const details = createManualConnectionDetails({
      appName: 'Switchify PC',
      connectionDetails: {
        desktopId: 'desktop-1',
        websocketUrl: 'ws://[2001:bb6:a61:3700:574c:69d2:25ce:505]:7347',
        websocketUrls: [
          'ws://[2001:bb6:a61:3700:574c:69d2:25ce:505]:7347',
          'ws://192.168.1.180:7347',
          'ws://127.0.0.1:7347',
          'ws://[::1]:7347'
        ]
      }
    });

    expect(details.payload).toEqual({
      type: 'switchify.pc.connect',
      version: 1,
      desktopId: 'desktop-1',
      displayName: 'Switchify PC',
      urls: [
        'ws://[2001:bb6:a61:3700:574c:69d2:25ce:505]:7347',
        'ws://192.168.1.180:7347'
      ]
    });
    expect(details.payloadJson).not.toContain('token');
    expect(details.payloadJson).not.toContain('auth');
    expect(details.payloadJson).not.toContain('secret');
    expect(details.payloadJson).not.toContain('nonce');
    expect(details.payloadJson).not.toContain('kind');
    expect(details.payloadJson).not.toContain('protocolVersion');
    expect(details.payloadJson).not.toContain('"name"');
  });

  it('returns no payload when only loopback URLs are available', () => {
    expect(
      createManualConnectionDetails({
        appName: 'Switchify PC',
        connectionDetails: {
          desktopId: 'desktop-1',
          websocketUrl: 'ws://127.0.0.1:7347',
          websocketUrls: ['ws://127.0.0.1:7347', 'ws://[::1]:7347']
        }
      })
    ).toEqual({ payload: null, payloadJson: null, urls: [] });
  });
});
