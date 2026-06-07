export function formatTimestamp(value: number | null): string {
  if (!value) return 'Not yet.';
  return new Intl.DateTimeFormat(undefined, {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  }).format(value);
}
