import type { ReactElement } from 'react';
import type { DesktopUiState } from '../../shared/desktop-ui-state';

export function StatusHeader({ state, appName }: { state: DesktopUiState; appName: string }): ReactElement {
  return (
    <header className="setup-header">
      <div>
        <p className="section-label">Desktop companion</p>
        <h1>{appName}</h1>
      </div>
      <div className={`status-badge status-badge-${statusTone(state)}`}>
        <span className="status-dot" />
        {statusBadgeLabel(state)}
      </div>
    </header>
  );
}

function statusTone(state: DesktopUiState): 'ready' | 'connected' | 'waiting' | 'error' {
  if (state === 'connected') return 'connected';
  if (state === 'server-error') return 'error';
  if (state === 'loading' || state === 'starting' || state === 'waiting-for-device' || state === 'not-running') {
    return 'waiting';
  }
  return 'ready';
}

function statusBadgeLabel(state: DesktopUiState): string {
  if (state === 'connected') return 'Connected';
  if (state === 'server-error') return 'Needs attention';
  if (state === 'waiting-for-device') return 'Waiting';
  if (state === 'not-running') return 'Not running';
  if (state === 'loading' || state === 'starting') return 'Starting...';
  return 'Ready';
}
