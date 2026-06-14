import type { ProtocolErrorCode, ProtocolResponse } from '../../shared/protocol';
import type { RemoteConnection } from './remote-connection';

export function toProtocolCommandErrorCode(
  code: 'unsupported_command' | 'unsafe_payload' | 'adapter_failure'
): ProtocolErrorCode {
  if (code === 'unsafe_payload') return 'invalid_payload';
  if (code === 'unsupported_command') return 'invalid_type';
  return 'command_failed';
}

export async function sendResponse(connection: RemoteConnection, response: ProtocolResponse): Promise<void> {
  await connection.send(JSON.stringify(response));
}

