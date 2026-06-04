import { describe, expect, it } from 'vitest';
import {
  createAckResponse,
  createErrorResponse,
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

  it('rejects malformed responses', () => {
    expect(validateProtocolResponse({ version: PROTOCOL_VERSION, id: 'request-1', type: 'ack', ok: false, error: null })).toMatchObject({
      ok: false,
      error: 'invalid_message'
    });
  });
});
