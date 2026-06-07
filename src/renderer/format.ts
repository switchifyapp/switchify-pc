export type CopyState = 'idle' | 'copied' | 'failed';

export function formatExpiry(value: number | null): string {
  if (!value) return 'No active code.';
  const remainingMs = Math.max(0, value - Date.now());
  const minutes = Math.floor(remainingMs / 60_000);
  const seconds = Math.floor((remainingMs % 60_000) / 1000);
  return `${minutes}m ${seconds.toString().padStart(2, '0')}s`;
}

export function formatTimestamp(value: number | null): string {
  if (!value) return 'Not yet.';
  return new Intl.DateTimeFormat(undefined, {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  }).format(value);
}

export function formatCopyState(state: CopyState): string {
  if (state === 'copied') return 'Copied.';
  if (state === 'failed') return 'Copy failed.';
  return '';
}
