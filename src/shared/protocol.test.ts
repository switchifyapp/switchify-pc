import { describe, expect, it } from 'vitest';
import {
  createAckResponse,
  createErrorResponse,
  createPairingCompleteResponse,
  createPointerProfileResponse,
  MAX_POINTER_DELTA,
  parseProtocolRequest,
  PROTOCOL_VERSION,
  validateProtocolRequest,
  validateProtocolResponse
} from './protocol';

const baseCommand = {
  version: PROTOCOL_VERSION,
  id: 'request-1',
  deviceId: 'android-device-1',
  timestamp: 1_724_000_000_000,
  auth: 'proof'
};

describe('protocol request validation', () => {
  it('accepts all MVP command payloads', () => {
    const commands = [
      { type: 'mouse.move', payload: { dx: 12, dy: -6 } },
      { type: 'mouse.click', payload: { button: 'left' } },
      { type: 'mouse.doubleClick', payload: { button: 'middle' } },
      { type: 'mouse.rightClick', payload: {} },
      { type: 'mouse.scroll', payload: { dx: 0, dy: -3 } },
      { type: 'keyboard.key', payload: { key: 'Enter' } },
      { type: 'keyboard.shortcut', payload: { keys: ['Ctrl', 'C'] } },
      { type: 'keyboard.typeText', payload: { text: 'Hello' } },
      { type: 'media.control', payload: { action: 'playPause' } },
      { type: 'pointer.profile', payload: {} },
      { type: 'connection.ping', payload: {} }
    ];

    for (const command of commands) {
      expect(validateProtocolRequest({ ...baseCommand, ...command })).toMatchObject({ ok: true });
    }
  });

  it('accepts pairing messages without command auth fields', () => {
    expect(
      validateProtocolRequest({
        version: PROTOCOL_VERSION,
        id: 'pairing-1',
        type: 'pairing.start',
        payload: {
          deviceId: 'android-device-1',
          deviceName: 'Android phone',
          pairingCode: '123456'
        }
      })
    ).toMatchObject({ ok: true });

    expect(
      validateProtocolRequest({
        version: PROTOCOL_VERSION,
        id: 'pairing-2',
        type: 'pairing.complete',
        payload: {
          deviceId: 'android-device-1',
          desktopId: 'desktop-1',
          pairingNonce: 'nonce'
        }
      })
    ).toMatchObject({ ok: true });

    expect(
      validateProtocolRequest({
        version: PROTOCOL_VERSION,
        id: 'pairing-3',
        type: 'pairing.request',
        payload: {
          deviceId: 'android-device-1',
          deviceName: 'Android phone',
          desktopId: 'desktop-1',
          requestNonce: 'nonce'
        }
      })
    ).toMatchObject({ ok: true });
  });

  it('rejects invalid pairing approval payloads', () => {
    const validPayload = {
      deviceId: 'android-device-1',
      deviceName: 'Android phone',
      desktopId: 'desktop-1',
      requestNonce: 'nonce'
    };

    for (const field of ['deviceId', 'deviceName', 'desktopId', 'requestNonce'] as const) {
      expect(
        validateProtocolRequest({
          version: PROTOCOL_VERSION,
          id: `pairing-request-missing-${field}`,
          type: 'pairing.request',
          payload: {
            ...validPayload,
            [field]: ''
          }
        })
      ).toMatchObject({ ok: false, error: 'invalid_payload' });
    }
  });

  it('rejects invalid JSON', () => {
    expect(parseProtocolRequest('{')).toMatchObject({
      ok: false,
      error: 'invalid_json'
    });
  });

  it('rejects missing envelope fields', () => {
    expect(validateProtocolRequest({ version: PROTOCOL_VERSION })).toMatchObject({
      ok: false,
      error: 'invalid_message'
    });
  });

  it('rejects unsupported versions and message types', () => {
    expect(validateProtocolRequest({ ...baseCommand, version: 99, type: 'connection.ping', payload: {} })).toMatchObject({
      ok: false,
      error: 'invalid_version'
    });
    expect(validateProtocolRequest({ ...baseCommand, type: 'desktop.shutdown', payload: {} })).toMatchObject({
      ok: false,
      error: 'invalid_type'
    });
  });

  it('rejects command messages without auth proof', () => {
    const { auth, ...withoutAuth } = baseCommand;

    expect(validateProtocolRequest({ ...withoutAuth, type: 'connection.ping', payload: {} })).toMatchObject({
      ok: false,
      error: 'invalid_auth'
    });
  });

  it('rejects unsafe command payloads', () => {
    expect(validateProtocolRequest({ ...baseCommand, type: 'mouse.move', payload: { dx: 501, dy: 0 } })).toMatchObject({
      ok: false,
      error: 'invalid_payload'
    });
    expect(validateProtocolRequest({ ...baseCommand, type: 'keyboard.shortcut', payload: { keys: [] } })).toMatchObject({
      ok: false,
      error: 'invalid_payload'
    });
    expect(validateProtocolRequest({ ...baseCommand, type: 'keyboard.typeText', payload: { text: 'x'.repeat(2_001) } })).toMatchObject({
      ok: false,
      error: 'invalid_payload'
    });
    expect(validateProtocolRequest({ ...baseCommand, type: 'pointer.profile', payload: { includeDisplays: true } })).toMatchObject({
      ok: false,
      error: 'invalid_payload'
    });
  });
});

describe('protocol response validation', () => {
  it('creates and validates ack responses', () => {
    const response = createAckResponse('request-1');

    expect(response).toEqual({
      version: PROTOCOL_VERSION,
      id: 'request-1',
      type: 'ack',
      ok: true,
      error: null
    });
    expect(validateProtocolResponse(response)).toMatchObject({ ok: true });
  });

  it('creates and validates structured error responses', () => {
    const response = createErrorResponse('request-1', 'invalid_payload', 'Payload rejected.');

    expect(response).toMatchObject({
      version: PROTOCOL_VERSION,
      id: 'request-1',
      type: 'error',
      ok: false,
      error: {
        code: 'invalid_payload',
        message: 'Payload rejected.'
      }
    });
    expect(validateProtocolResponse(response)).toMatchObject({ ok: true });
  });

  it('creates and validates pairing complete responses', () => {
    const response = createPairingCompleteResponse('pairing-1', {
      desktopId: 'desktop-1',
      deviceId: 'android-1',
      token: 'paired-token'
    });

    expect(validateProtocolResponse(response)).toMatchObject({ ok: true });
  });

  it('creates and validates pointer profile responses', () => {
    const response = createPointerProfileResponse('profile-1', {
      displayId: '0:0:1280:720:1.5',
      scaleFactor: 1.5,
      bounds: { x: 0, y: 0, width: 1280, height: 720 },
      maxDelta: MAX_POINTER_DELTA,
      recommendedDeltas: {
        small: 50,
        medium: 130,
        large: 252
      }
    });

    expect(validateProtocolResponse(response)).toMatchObject({ ok: true });
  });

  it('rejects malformed pointer profile responses', () => {
    expect(
      validateProtocolResponse({
        version: PROTOCOL_VERSION,
        id: 'profile-1',
        type: 'pointer.profile',
        ok: true,
        payload: {
          displayId: '0:0:1280:720:1.5',
          scaleFactor: 0,
          bounds: { x: 0, y: 0, width: 1280, height: 720 },
          maxDelta: MAX_POINTER_DELTA,
          recommendedDeltas: { small: 50, medium: 130, large: 252 }
        },
        error: null
      })
    ).toMatchObject({ ok: false, error: 'invalid_payload' });

    expect(
      validateProtocolResponse({
        version: PROTOCOL_VERSION,
        id: 'profile-1',
        type: 'pointer.profile',
        ok: true,
        payload: {
          displayId: '0:0:1280:720:1.5',
          scaleFactor: 1.5,
          bounds: { x: 0, y: 0, width: 1280, height: 720 },
          maxDelta: MAX_POINTER_DELTA
        },
        error: null
      })
    ).toMatchObject({ ok: false, error: 'invalid_payload' });
  });

  it('rejects malformed responses', () => {
    expect(validateProtocolResponse({ version: PROTOCOL_VERSION, id: 'request-1', type: 'ack', ok: false, error: null })).toMatchObject({
      ok: false,
      error: 'invalid_message'
    });
  });
});
