import { WebSocket } from 'ws';
import type { ProtocolErrorCode, ProtocolResponse } from '../../shared/protocol';

export function toProtocolCommandErrorCode(
  code: 'unsupported_command' | 'unsafe_payload' | 'adapter_failure'
): ProtocolErrorCode {
  if (code === 'unsafe_payload') return 'invalid_payload';
  if (code === 'unsupported_command') return 'invalid_type';
  return 'command_failed';
}

export function sendResponse(client: WebSocket, response: ProtocolResponse): void {
  if (client.readyState === WebSocket.OPEN) {
    client.send(JSON.stringify(response));
  }
}
