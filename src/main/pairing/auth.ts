import { createHmac, timingSafeEqual } from 'node:crypto';
import type { CommandRequest, CommandResponseMode } from '../../shared/protocol';
import { validateProtocolRequest } from '../../shared/protocol';
import type { PairingStore } from './pairing-store';
import { findPairedDevice } from './pairing-store';

export const COMMAND_TIMESTAMP_TOLERANCE_MS = 2 * 60 * 1000;
export const REPLAY_CACHE_TTL_MS = COMMAND_TIMESTAMP_TOLERANCE_MS;

export type AuthValidationResult =
  | { ok: true; command: CommandRequest }
  | { ok: false; reason: 'invalid_payload' | 'unknown_device' | 'expired_timestamp' | 'duplicate_request' | 'invalid_auth' };

export class CommandAuthValidator {
  private readonly seenRequestIds = new Map<string, number>();

  constructor(
    private readonly store: PairingStore,
    private readonly now: () => number = Date.now
  ) {}

  async validate(value: unknown): Promise<AuthValidationResult> {
    const parsed = validateProtocolRequest(value);
    if (!parsed.ok || !isCommandRequest(parsed.value)) {
      return { ok: false, reason: 'invalid_payload' };
    }

    const command = parsed.value;
    const state = await this.store.load();
    const pairedDevice = findPairedDevice(state, command.deviceId);
    if (!pairedDevice) return { ok: false, reason: 'unknown_device' };
    if (Math.abs(this.now() - command.timestamp) > COMMAND_TIMESTAMP_TOLERANCE_MS) {
      return { ok: false, reason: 'expired_timestamp' };
    }
    this.pruneReplayCache();
    if (this.seenRequestIds.has(replayKey(command))) {
      return { ok: false, reason: 'duplicate_request' };
    }

    const expectedAuth = createCommandAuthProof(command, pairedDevice.token);
    if (!safeEquals(command.auth, expectedAuth)) {
      return { ok: false, reason: 'invalid_auth' };
    }

    this.seenRequestIds.set(replayKey(command), this.now() + REPLAY_CACHE_TTL_MS);
    pairedDevice.lastSeenAt = this.now();
    await this.store.save(state);

    return { ok: true, command };
  }

  private pruneReplayCache(): void {
    const now = this.now();
    for (const [key, expiresAt] of this.seenRequestIds.entries()) {
      if (expiresAt <= now) {
        this.seenRequestIds.delete(key);
      }
    }
  }
}

export function createCommandAuthProof(command: CommandRequest, token: string): string {
  return createHmac('sha256', token).update(canonicalCommandString(command)).digest('base64url');
}

function canonicalCommandString(command: CommandRequest): string {
  return [
    command.version,
    command.id,
    command.deviceId,
    command.timestamp,
    command.type,
    stableStringify(command.payload),
    commandResponseMode(command)
  ].join('\n');
}

function commandResponseMode(command: CommandRequest): CommandResponseMode {
  return command.responseMode ?? 'ack';
}

function stableStringify(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map(stableStringify).join(',')}]`;
  }
  if (value && typeof value === 'object') {
    const record = value as Record<string, unknown>;
    return `{${Object.keys(record)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${stableStringify(record[key])}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
}

function isCommandRequest(value: unknown): value is CommandRequest {
  return (
    typeof value === 'object' &&
    value !== null &&
    'auth' in value &&
    'deviceId' in value &&
    'timestamp' in value
  );
}

function safeEquals(actual: string, expected: string): boolean {
  const actualBuffer = Buffer.from(actual);
  const expectedBuffer = Buffer.from(expected);
  return actualBuffer.length === expectedBuffer.length && timingSafeEqual(actualBuffer, expectedBuffer);
}

function replayKey(command: CommandRequest): string {
  return `${command.deviceId}:${command.id}`;
}
