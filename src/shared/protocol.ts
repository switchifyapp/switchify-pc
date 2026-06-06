export const PROTOCOL_VERSION = 1;

export const MAX_TEXT_LENGTH = 2_000;
export const MAX_POINTER_DELTA = 500;
export const MAX_SCROLL_DELTA = 50;
export const MAX_SHORTCUT_KEYS = 6;
export const MAX_ERROR_MESSAGE_LENGTH = 300;

export type ProtocolErrorCode =
  | 'invalid_json'
  | 'invalid_message'
  | 'invalid_version'
  | 'invalid_type'
  | 'invalid_payload'
  | 'invalid_auth'
  | 'command_failed';

export type MouseButton = 'left' | 'right' | 'middle';

export type KeyboardKey =
  | 'Backspace'
  | 'Delete'
  | 'Enter'
  | 'Escape'
  | 'Space'
  | 'Tab'
  | 'ArrowUp'
  | 'ArrowDown'
  | 'ArrowLeft'
  | 'ArrowRight'
  | 'Home'
  | 'End'
  | 'PageUp'
  | 'PageDown';

export type ShortcutKey =
  | KeyboardKey
  | 'Ctrl'
  | 'Alt'
  | 'Shift'
  | 'Meta'
  | 'A'
  | 'C'
  | 'V'
  | 'X'
  | 'Z'
  | 'Y';

export type MediaAction = 'playPause' | 'nextTrack' | 'previousTrack' | 'volumeUp' | 'volumeDown' | 'mute';

export interface BaseRequestEnvelope<TType extends string, TPayload> {
  version: typeof PROTOCOL_VERSION;
  id: string;
  deviceId: string;
  timestamp: number;
  type: TType;
  payload: TPayload;
  auth: string;
}

export type PairingStartRequest = {
  version: typeof PROTOCOL_VERSION;
  id: string;
  type: 'pairing.start';
  payload: {
    deviceId: string;
    deviceName: string;
    pairingCode: string;
  };
};

export type PairingCompleteRequest = {
  version: typeof PROTOCOL_VERSION;
  id: string;
  type: 'pairing.complete';
  payload: {
    deviceId: string;
    desktopId: string;
    pairingNonce: string;
  };
};

export type PairingApprovalRequest = {
  version: typeof PROTOCOL_VERSION;
  id: string;
  type: 'pairing.request';
  payload: {
    deviceId: string;
    deviceName: string;
    desktopId: string;
    requestNonce: string;
  };
};

export type PairingRequest = PairingStartRequest | PairingCompleteRequest | PairingApprovalRequest;

export type MouseMoveCommand = BaseRequestEnvelope<'mouse.move', { dx: number; dy: number }>;
export type MouseClickCommand = BaseRequestEnvelope<'mouse.click', { button: MouseButton }>;
export type MouseDoubleClickCommand = BaseRequestEnvelope<'mouse.doubleClick', { button: MouseButton }>;
export type MouseRightClickCommand = BaseRequestEnvelope<'mouse.rightClick', Record<string, never>>;
export type MouseScrollCommand = BaseRequestEnvelope<'mouse.scroll', { dx: number; dy: number }>;
export type KeyboardKeyCommand = BaseRequestEnvelope<'keyboard.key', { key: KeyboardKey }>;
export type KeyboardShortcutCommand = BaseRequestEnvelope<'keyboard.shortcut', { keys: ShortcutKey[] }>;
export type KeyboardTypeTextCommand = BaseRequestEnvelope<'keyboard.typeText', { text: string }>;
export type MediaControlCommand = BaseRequestEnvelope<'media.control', { action: MediaAction }>;
export type PingCommand = BaseRequestEnvelope<'connection.ping', Record<string, never>>;

export type CommandRequest =
  | MouseMoveCommand
  | MouseClickCommand
  | MouseDoubleClickCommand
  | MouseRightClickCommand
  | MouseScrollCommand
  | KeyboardKeyCommand
  | KeyboardShortcutCommand
  | KeyboardTypeTextCommand
  | MediaControlCommand
  | PingCommand;

export type ProtocolRequest = CommandRequest | PairingRequest;

export type AckResponse = {
  version: typeof PROTOCOL_VERSION;
  id: string;
  type: 'ack';
  ok: true;
  error: null;
};

export type ErrorResponse = {
  version: typeof PROTOCOL_VERSION;
  id: string | null;
  type: 'error';
  ok: false;
  error: {
    code: ProtocolErrorCode;
    message: string;
    detail?: string;
  };
};

export type PairingCompleteResponse = {
  version: typeof PROTOCOL_VERSION;
  id: string;
  type: 'pairing.complete';
  ok: true;
  payload: {
    desktopId: string;
    deviceId: string;
    token: string;
  };
  error: null;
};

export type ProtocolResponse = AckResponse | ErrorResponse | PairingCompleteResponse;

export type ValidationResult<T> =
  | { ok: true; value: T }
  | { ok: false; error: ProtocolErrorCode; message: string };

const commandTypes = new Set<CommandRequest['type']>([
  'mouse.move',
  'mouse.click',
  'mouse.doubleClick',
  'mouse.rightClick',
  'mouse.scroll',
  'keyboard.key',
  'keyboard.shortcut',
  'keyboard.typeText',
  'media.control',
  'connection.ping'
]);

const pairingTypes = new Set<PairingRequest['type']>(['pairing.start', 'pairing.complete', 'pairing.request']);
const mouseButtons = new Set<MouseButton>(['left', 'right', 'middle']);
const keyboardKeys = new Set<KeyboardKey>([
  'Backspace',
  'Delete',
  'Enter',
  'Escape',
  'Space',
  'Tab',
  'ArrowUp',
  'ArrowDown',
  'ArrowLeft',
  'ArrowRight',
  'Home',
  'End',
  'PageUp',
  'PageDown'
]);
const shortcutKeys = new Set<ShortcutKey>([
  ...keyboardKeys,
  'Ctrl',
  'Alt',
  'Shift',
  'Meta',
  'A',
  'C',
  'V',
  'X',
  'Z',
  'Y'
]);
const mediaActions = new Set<MediaAction>([
  'playPause',
  'nextTrack',
  'previousTrack',
  'volumeUp',
  'volumeDown',
  'mute'
]);

export function parseProtocolRequest(raw: string): ValidationResult<ProtocolRequest> {
  try {
    return validateProtocolRequest(JSON.parse(raw));
  } catch {
    return invalid('invalid_json', 'Message must be valid JSON.');
  }
}

export function validateProtocolRequest(value: unknown): ValidationResult<ProtocolRequest> {
  if (!isRecord(value)) return invalid('invalid_message', 'Message must be an object.');
  if (value.version !== PROTOCOL_VERSION) return invalid('invalid_version', 'Unsupported protocol version.');
  if (!isNonEmptyString(value.id)) return invalid('invalid_message', 'Message id is required.');
  if (!isNonEmptyString(value.type)) return invalid('invalid_type', 'Message type is required.');
  if (!('payload' in value) || !isRecord(value.payload)) {
    return invalid('invalid_payload', 'Payload must be an object.');
  }

  if (commandTypes.has(value.type as CommandRequest['type'])) {
    return validateCommandRequest(value);
  }

  if (pairingTypes.has(value.type as PairingRequest['type'])) {
    return validatePairingRequest(value);
  }

  return invalid('invalid_type', 'Unsupported message type.');
}

export function validateProtocolResponse(value: unknown): ValidationResult<ProtocolResponse> {
  if (!isRecord(value)) return invalid('invalid_message', 'Response must be an object.');
  if (value.version !== PROTOCOL_VERSION) return invalid('invalid_version', 'Unsupported protocol version.');
  if (value.type === 'ack') {
    if (!isNonEmptyString(value.id)) return invalid('invalid_message', 'Ack id is required.');
    if (value.ok !== true || value.error !== null) return invalid('invalid_message', 'Ack response is malformed.');
    return { ok: true, value: value as AckResponse };
  }

  if (value.type === 'error') {
    if (!(value.id === null || isNonEmptyString(value.id))) {
      return invalid('invalid_message', 'Error response id must be a string or null.');
    }
    if (value.ok !== false || !isRecord(value.error)) {
      return invalid('invalid_message', 'Error response is malformed.');
    }
    if (!isNonEmptyString(value.error.code) || !isNonEmptyString(value.error.message)) {
      return invalid('invalid_message', 'Error code and message are required.');
    }
    if (value.error.message.length > MAX_ERROR_MESSAGE_LENGTH) {
      return invalid('invalid_message', 'Error message is too long.');
    }
    return { ok: true, value: value as ErrorResponse };
  }

  if (value.type === 'pairing.complete') {
    if (!isNonEmptyString(value.id)) return invalid('invalid_message', 'Pairing response id is required.');
    if (value.ok !== true || value.error !== null || !isRecord(value.payload)) {
      return invalid('invalid_message', 'Pairing response is malformed.');
    }
    if (
      !isNonEmptyString(value.payload.desktopId) ||
      !isNonEmptyString(value.payload.deviceId) ||
      !isNonEmptyString(value.payload.token)
    ) {
      return invalid('invalid_payload', 'Pairing response payload is invalid.');
    }
    return { ok: true, value: value as PairingCompleteResponse };
  }

  return invalid('invalid_type', 'Unsupported response type.');
}

export function createAckResponse(id: string): AckResponse {
  return {
    version: PROTOCOL_VERSION,
    id,
    type: 'ack',
    ok: true,
    error: null
  };
}

export function createErrorResponse(
  id: string | null,
  code: ProtocolErrorCode,
  message: string,
  detail?: string
): ErrorResponse {
  return {
    version: PROTOCOL_VERSION,
    id,
    type: 'error',
    ok: false,
    error: {
      code,
      message: message.slice(0, MAX_ERROR_MESSAGE_LENGTH),
      ...(detail ? { detail } : {})
    }
  };
}

export function createPairingCompleteResponse(
  id: string,
  payload: PairingCompleteResponse['payload']
): PairingCompleteResponse {
  return {
    version: PROTOCOL_VERSION,
    id,
    type: 'pairing.complete',
    ok: true,
    payload,
    error: null
  };
}

function validateCommandRequest(value: Record<string, unknown>): ValidationResult<CommandRequest> {
  if (!isNonEmptyString(value.deviceId)) return invalid('invalid_message', 'Device id is required.');
  if (!isFiniteNumber(value.timestamp)) return invalid('invalid_message', 'Timestamp is required.');
  if (!isNonEmptyString(value.auth)) return invalid('invalid_auth', 'Auth proof is required.');

  const payload = value.payload as Record<string, unknown>;
  const payloadOk = validateCommandPayload(value.type as CommandRequest['type'], payload);
  if (!payloadOk.ok) return payloadOk;

  return { ok: true, value: value as unknown as CommandRequest };
}

function validatePairingRequest(value: Record<string, unknown>): ValidationResult<PairingRequest> {
  const payload = value.payload as Record<string, unknown>;

  if (value.type === 'pairing.start') {
    if (!isNonEmptyString(payload.deviceId)) return invalid('invalid_payload', 'Pairing device id is required.');
    if (!isNonEmptyString(payload.deviceName)) return invalid('invalid_payload', 'Pairing device name is required.');
    if (!isNonEmptyString(payload.pairingCode)) return invalid('invalid_payload', 'Pairing code is required.');
    return { ok: true, value: value as PairingStartRequest };
  }

  if (value.type === 'pairing.request') {
    if (!isNonEmptyString(payload.deviceId)) return invalid('invalid_payload', 'Pairing device id is required.');
    if (!isNonEmptyString(payload.deviceName)) return invalid('invalid_payload', 'Pairing device name is required.');
    if (!isNonEmptyString(payload.desktopId)) return invalid('invalid_payload', 'Desktop id is required.');
    if (!isNonEmptyString(payload.requestNonce)) return invalid('invalid_payload', 'Pairing request nonce is required.');
    return { ok: true, value: value as PairingApprovalRequest };
  }

  if (!isNonEmptyString(payload.deviceId)) return invalid('invalid_payload', 'Pairing device id is required.');
  if (!isNonEmptyString(payload.desktopId)) return invalid('invalid_payload', 'Desktop id is required.');
  if (!isNonEmptyString(payload.pairingNonce)) return invalid('invalid_payload', 'Pairing nonce is required.');
  return { ok: true, value: value as PairingCompleteRequest };
}

function validateCommandPayload(
  type: CommandRequest['type'],
  payload: Record<string, unknown>
): ValidationResult<unknown> {
  switch (type) {
    case 'mouse.move':
      return validateBoundedNumbers(payload, ['dx', 'dy'], MAX_POINTER_DELTA);
    case 'mouse.scroll':
      return validateBoundedNumbers(payload, ['dx', 'dy'], MAX_SCROLL_DELTA);
    case 'mouse.click':
    case 'mouse.doubleClick':
      return mouseButtons.has(payload.button as MouseButton)
        ? valid()
        : invalid('invalid_payload', 'Mouse button is invalid.');
    case 'mouse.rightClick':
    case 'connection.ping':
      return Object.keys(payload).length === 0
        ? valid()
        : invalid('invalid_payload', 'Payload must be empty.');
    case 'keyboard.key':
      return keyboardKeys.has(payload.key as KeyboardKey)
        ? valid()
        : invalid('invalid_payload', 'Keyboard key is invalid.');
    case 'keyboard.shortcut':
      return validateShortcutPayload(payload);
    case 'keyboard.typeText':
      return isString(payload.text) && payload.text.length <= MAX_TEXT_LENGTH
        ? valid()
        : invalid('invalid_payload', 'Text payload is invalid.');
    case 'media.control':
      return mediaActions.has(payload.action as MediaAction)
        ? valid()
        : invalid('invalid_payload', 'Media action is invalid.');
  }
}

function validateShortcutPayload(payload: Record<string, unknown>): ValidationResult<unknown> {
  if (!Array.isArray(payload.keys)) {
    return invalid('invalid_payload', 'Shortcut keys must be an array.');
  }
  if (payload.keys.length === 0 || payload.keys.length > MAX_SHORTCUT_KEYS) {
    return invalid('invalid_payload', 'Shortcut key count is invalid.');
  }
  if (!payload.keys.every((key) => shortcutKeys.has(key as ShortcutKey))) {
    return invalid('invalid_payload', 'Shortcut contains an invalid key.');
  }
  return valid();
}

function validateBoundedNumbers(
  payload: Record<string, unknown>,
  keys: string[],
  maxAbsValue: number
): ValidationResult<unknown> {
  for (const key of keys) {
    const value = payload[key];
    if (!isFiniteNumber(value) || Math.abs(value) > maxAbsValue) {
      return invalid('invalid_payload', `${key} is invalid.`);
    }
  }
  return valid();
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isString(value: unknown): value is string {
  return typeof value === 'string';
}

function isNonEmptyString(value: unknown): value is string {
  return isString(value) && value.length > 0;
}

function isFiniteNumber(value: unknown): value is number {
  return typeof value === 'number' && Number.isFinite(value);
}

function valid(): ValidationResult<unknown> {
  return { ok: true, value: undefined };
}

function invalid(error: ProtocolErrorCode, message: string): ValidationResult<never> {
  return { ok: false, error, message };
}
