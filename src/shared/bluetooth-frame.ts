import { randomUUID } from 'node:crypto';

export const BLUETOOTH_FRAME_VERSION = 1;
export const DEFAULT_BLUETOOTH_FRAME_PAYLOAD_BYTES = 160;
export const DEFAULT_BLUETOOTH_MAX_MESSAGE_BYTES = 16 * 1024;
export const DEFAULT_BLUETOOTH_PARTIAL_TIMEOUT_MS = 10_000;

export type BluetoothFrame = {
  version: typeof BLUETOOTH_FRAME_VERSION;
  messageId: string;
  sequence: number;
  isFinal: boolean;
  totalBytes: number;
  payloadBase64: string;
};

export type BluetoothFrameReassemblyResult =
  | { ok: true; message: string }
  | { ok: false; reason: 'incomplete' | 'invalid_frame' | 'message_too_large' | 'expired' };

type PartialBluetoothMessage = {
  totalBytes: number;
  createdAt: number;
  chunks: Map<number, Buffer>;
};

export function createBluetoothFrames(
  message: string,
  options: { messageId?: string; maxPayloadBytes?: number; maxMessageBytes?: number } = {}
): BluetoothFrame[] {
  const payload = Buffer.from(message, 'utf8');
  const maxMessageBytes = options.maxMessageBytes ?? DEFAULT_BLUETOOTH_MAX_MESSAGE_BYTES;
  const maxPayloadBytes = options.maxPayloadBytes ?? DEFAULT_BLUETOOTH_FRAME_PAYLOAD_BYTES;

  if (!Number.isInteger(maxPayloadBytes) || maxPayloadBytes <= 0) {
    throw new Error('Bluetooth frame payload size must be positive.');
  }
  if (payload.byteLength > maxMessageBytes) {
    throw new Error('Bluetooth message is too large.');
  }

  const messageId = options.messageId ?? randomUUID();
  const frames: BluetoothFrame[] = [];
  for (let offset = 0, sequence = 0; offset < payload.byteLength || sequence === 0; offset += maxPayloadBytes, sequence += 1) {
    const chunk = payload.subarray(offset, Math.min(payload.byteLength, offset + maxPayloadBytes));
    frames.push({
      version: BLUETOOTH_FRAME_VERSION,
      messageId,
      sequence,
      isFinal: offset + maxPayloadBytes >= payload.byteLength,
      totalBytes: payload.byteLength,
      payloadBase64: chunk.toString('base64')
    });
  }
  return frames;
}

export class BluetoothFrameReassembler {
  private readonly partialMessages = new Map<string, PartialBluetoothMessage>();

  constructor(
    private readonly options: {
      maxMessageBytes?: number;
      partialTimeoutMs?: number;
      now?: () => number;
    } = {}
  ) {}

  accept(frame: BluetoothFrame): BluetoothFrameReassemblyResult {
    const validation = getBluetoothFrameValidationError(frame, this.options.maxMessageBytes);
    if (validation) return validation;

    this.expirePartials();

    const now = this.now();
    const existing = this.partialMessages.get(frame.messageId);
    const partial =
      existing ??
      ({
        totalBytes: frame.totalBytes,
        createdAt: now,
        chunks: new Map<number, Buffer>()
      } satisfies PartialBluetoothMessage);

    if (partial.totalBytes !== frame.totalBytes) {
      this.partialMessages.delete(frame.messageId);
      return { ok: false, reason: 'invalid_frame' };
    }

    if (!partial.chunks.has(frame.sequence)) {
      partial.chunks.set(frame.sequence, Buffer.from(frame.payloadBase64, 'base64'));
    }
    this.partialMessages.set(frame.messageId, partial);

    if (!frame.isFinal) return { ok: false, reason: 'incomplete' };

    const chunks: Buffer[] = [];
    let totalBytes = 0;
    for (let sequence = 0; partial.chunks.has(sequence); sequence += 1) {
      const chunk = partial.chunks.get(sequence);
      if (!chunk) return { ok: false, reason: 'incomplete' };
      chunks.push(chunk);
      totalBytes += chunk.byteLength;
      if (totalBytes > partial.totalBytes) {
        this.partialMessages.delete(frame.messageId);
        return { ok: false, reason: 'invalid_frame' };
      }
    }

    if (totalBytes !== partial.totalBytes) return { ok: false, reason: 'incomplete' };

    this.partialMessages.delete(frame.messageId);
    return { ok: true, message: Buffer.concat(chunks, totalBytes).toString('utf8') };
  }

  clearExpired(): number {
    return this.expirePartials();
  }

  private expirePartials(): number {
    const deadline = this.now() - (this.options.partialTimeoutMs ?? DEFAULT_BLUETOOTH_PARTIAL_TIMEOUT_MS);
    let expiredCount = 0;
    for (const [messageId, partial] of this.partialMessages) {
      if (partial.createdAt <= deadline) {
        this.partialMessages.delete(messageId);
        expiredCount += 1;
      }
    }
    return expiredCount;
  }

  private now(): number {
    return this.options.now?.() ?? Date.now();
  }
}

export function validateBluetoothFrame(
  frame: BluetoothFrame,
  maxMessageBytes = DEFAULT_BLUETOOTH_MAX_MESSAGE_BYTES
): BluetoothFrameReassemblyResult {
  return getBluetoothFrameValidationError(frame, maxMessageBytes) ?? { ok: false, reason: 'incomplete' };
}

function getBluetoothFrameValidationError(
  frame: BluetoothFrame,
  maxMessageBytes = DEFAULT_BLUETOOTH_MAX_MESSAGE_BYTES
): BluetoothFrameReassemblyResult | null {
  if (
    frame.version !== BLUETOOTH_FRAME_VERSION ||
    typeof frame.messageId !== 'string' ||
    frame.messageId.length === 0 ||
    !Number.isInteger(frame.sequence) ||
    frame.sequence < 0 ||
    typeof frame.isFinal !== 'boolean' ||
    !Number.isInteger(frame.totalBytes) ||
    frame.totalBytes < 0 ||
    frame.totalBytes > maxMessageBytes ||
    typeof frame.payloadBase64 !== 'string'
  ) {
    return { ok: false, reason: frame.totalBytes > maxMessageBytes ? 'message_too_large' : 'invalid_frame' };
  }

  if (!isValidBase64(frame.payloadBase64)) {
    return { ok: false, reason: 'invalid_frame' };
  }
  return null;
}

function isValidBase64(value: string): boolean {
  if (value.length === 0) return true;
  if (value.length % 4 !== 0) return false;
  return /^[A-Za-z0-9+/]+={0,2}$/.test(value);
}
