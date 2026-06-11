export const SWITCHIFY_MANUAL_CONNECTION_TYPE = 'switchify.pc.connect';

export type ManualConnectionPayload = {
  type: typeof SWITCHIFY_MANUAL_CONNECTION_TYPE;
  version: 1;
  desktopId: string;
  displayName: string;
  urls: string[];
};

const FORBIDDEN_KEY_PATTERN = /(token|auth|secret|nonce)/i;

export function createManualConnectionPayload(input: {
  desktopId: string;
  displayName: string;
  urls: string[];
}): ManualConnectionPayload {
  const payload: ManualConnectionPayload = {
    type: SWITCHIFY_MANUAL_CONNECTION_TYPE,
    version: 1,
    desktopId: input.desktopId.trim(),
    displayName: input.displayName.trim() || 'Switchify PC',
    urls: [...input.urls]
  };

  if (!validateManualConnectionPayload(payload)) {
    throw new Error('Invalid manual connection payload');
  }

  return payload;
}

export function validateManualConnectionPayload(value: unknown): value is ManualConnectionPayload {
  if (!isRecord(value) || hasForbiddenKey(value)) return false;
  if (value.type !== SWITCHIFY_MANUAL_CONNECTION_TYPE) return false;
  if (value.version !== 1) return false;
  if (typeof value.desktopId !== 'string' || value.desktopId.trim().length === 0) return false;
  if (typeof value.displayName !== 'string') return false;
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
