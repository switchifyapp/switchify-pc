export const SWITCHIFY_MANUAL_CONNECTION_KIND = 'switchify.pc.manual';

export type ManualConnectionPayload = {
  kind: typeof SWITCHIFY_MANUAL_CONNECTION_KIND;
  version: 1;
  protocolVersion: 1;
  desktopId: string;
  name: string;
  urls: string[];
};

const FORBIDDEN_KEY_PATTERN = /(token|auth|secret|nonce)/i;

export function createManualConnectionPayload(input: {
  desktopId: string;
  name: string;
  urls: string[];
}): ManualConnectionPayload {
  const payload: ManualConnectionPayload = {
    kind: SWITCHIFY_MANUAL_CONNECTION_KIND,
    version: 1,
    protocolVersion: 1,
    desktopId: input.desktopId.trim(),
    name: input.name.trim() || 'Switchify PC',
    urls: [...input.urls]
  };

  if (!validateManualConnectionPayload(payload)) {
    throw new Error('Invalid manual connection payload');
  }

  return payload;
}

export function validateManualConnectionPayload(value: unknown): value is ManualConnectionPayload {
  if (!isRecord(value) || hasForbiddenKey(value)) return false;
  if (value.kind !== SWITCHIFY_MANUAL_CONNECTION_KIND) return false;
  if (value.version !== 1 || value.protocolVersion !== 1) return false;
  if (typeof value.desktopId !== 'string' || value.desktopId.trim().length === 0) return false;
  if (typeof value.name !== 'string') return false;
  if (!Array.isArray(value.urls) || value.urls.length === 0) return false;

  return value.urls.every((url) => typeof url === 'string' && isValidWebSocketUrl(url));
}

function isValidWebSocketUrl(url: string): boolean {
  try {
    return new URL(url).protocol === 'ws:';
  } catch {
    return false;
  }
}

function hasForbiddenKey(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(hasForbiddenKey);
  if (!isRecord(value)) return false;

  return Object.entries(value).some(([key, child]) => FORBIDDEN_KEY_PATTERN.test(key) || hasForbiddenKey(child));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}
