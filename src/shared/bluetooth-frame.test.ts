import { describe, expect, it } from 'vitest';
import {
  BLUETOOTH_FRAME_VERSION,
  BluetoothFrameReassembler,
  createBluetoothFrames,
  validateBluetoothFrame,
  type BluetoothFrame
} from './bluetooth-frame';

describe('Bluetooth frames', () => {
  it('round trips a single-frame message', () => {
    const [frame] = createBluetoothFrames('{"type":"connection.ping"}', {
      messageId: 'message-1',
      maxPayloadBytes: 512
    });
    const reassembler = new BluetoothFrameReassembler();

    expect(reassembler.accept(frame)).toEqual({ ok: true, message: '{"type":"connection.ping"}' });
  });

  it('round trips a multi-frame message', () => {
    const frames = createBluetoothFrames('abcdefghijklmnopqrstuvwxyz', {
      messageId: 'message-1',
      maxPayloadBytes: 5
    });
    const reassembler = new BluetoothFrameReassembler();

    for (const frame of frames.slice(0, -1)) {
      expect(reassembler.accept(frame)).toEqual({ ok: false, reason: 'incomplete' });
    }
    expect(reassembler.accept(frames.at(-1)!)).toEqual({ ok: true, message: 'abcdefghijklmnopqrstuvwxyz' });
  });

  it('waits for missing chunks instead of reassembling partial messages', () => {
    const frames = createBluetoothFrames('abcdefghijklmnopqrstuvwxyz', {
      messageId: 'message-1',
      maxPayloadBytes: 5
    });
    const reassembler = new BluetoothFrameReassembler();

    expect(reassembler.accept(frames[0])).toEqual({ ok: false, reason: 'incomplete' });
    expect(reassembler.accept(frames.at(-1)!)).toEqual({ ok: false, reason: 'incomplete' });
  });

  it('rejects oversized messages', () => {
    expect(() => createBluetoothFrames('too large', { maxMessageBytes: 3 })).toThrow('Bluetooth message is too large.');
    expect(validateBluetoothFrame(createFrame({ totalBytes: 10 }), 3)).toEqual({
      ok: false,
      reason: 'message_too_large'
    });
  });

  it('rejects invalid frame versions', () => {
    expect(validateBluetoothFrame(createFrame({ version: 2 as typeof BLUETOOTH_FRAME_VERSION }))).toEqual({
      ok: false,
      reason: 'invalid_frame'
    });
  });

  it('expires partial messages', () => {
    let now = 1_000;
    const [first] = createBluetoothFrames('abcdefghijklmnopqrstuvwxyz', {
      messageId: 'message-1',
      maxPayloadBytes: 5
    });
    const reassembler = new BluetoothFrameReassembler({ now: () => now, partialTimeoutMs: 100 });

    expect(reassembler.accept(first)).toEqual({ ok: false, reason: 'incomplete' });
    now = 1_101;

    expect(reassembler.clearExpired()).toBe(1);
  });
});

function createFrame(overrides: Partial<BluetoothFrame> = {}): BluetoothFrame {
  return {
    version: BLUETOOTH_FRAME_VERSION,
    messageId: 'message-1',
    sequence: 0,
    isFinal: true,
    totalBytes: 3,
    payloadBase64: Buffer.from('abc').toString('base64'),
    ...overrides
  };
}

