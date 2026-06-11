import type { ConnectionDetails } from '../shared/server-status';
import {
  createManualConnectionPayload,
  type ManualConnectionPayload
} from '../shared/manual-connection';

export function nonLoopbackWebSocketUrls(urls: string[]): string[] {
  return urls.filter((url) => {
    try {
      const parsed = new URL(url);
      return parsed.protocol === 'ws:' && parsed.hostname !== '127.0.0.1' && parsed.hostname !== '[::1]' && parsed.hostname !== '::1';
    } catch {
      return false;
    }
  });
}

export function createManualConnectionDetails(input: {
  appName: string;
  connectionDetails: ConnectionDetails;
}): {
  payload: ManualConnectionPayload | null;
  payloadJson: string | null;
  urls: string[];
} {
  const urls = nonLoopbackWebSocketUrls(input.connectionDetails.websocketUrls);
  if (urls.length === 0) {
    return { payload: null, payloadJson: null, urls };
  }

  const payload = createManualConnectionPayload({
    desktopId: input.connectionDetails.desktopId,
    name: input.appName,
    urls
  });

  return {
    payload,
    payloadJson: JSON.stringify(payload),
    urls
  };
}
