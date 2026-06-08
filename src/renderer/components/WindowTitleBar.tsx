import type { ReactElement } from 'react';
import type { DesktopUiState } from '../../shared/desktop-ui-state';

export function WindowTitleBar({
  appName,
  state
}: {
  appName: string;
  state: DesktopUiState;
}): ReactElement {
  return (
    <header className="window-titlebar" aria-label="Window title bar">
      <div className="window-titlebar-main">
        <span className="window-titlebar-app">{appName}</span>
        <span className={`window-titlebar-status window-titlebar-status-${statusTone(state)}`}>
          <span className="window-titlebar-status-dot" />
          {statusLabel(state)}
        </span>
      </div>
    </header>
  );
}

function statusLabel(state: DesktopUiState): string {
  switch (state) {
    case 'loading':
    case 'starting':
      return 'Starting...';
    case 'not-running':
      return 'Not running';
    case 'server-error':
      return 'Needs attention';
    case 'waiting-for-device':
      return 'Waiting';
    case 'connected':
      return 'Connected';
    case 'ready-to-pair':
      return 'Ready';
  }
}

function statusTone(state: DesktopUiState): 'ready' | 'connected' | 'waiting' | 'error' {
  if (state === 'connected') return 'connected';
  if (state === 'server-error') return 'error';
  if (state === 'loading' || state === 'starting' || state === 'waiting-for-device' || state === 'not-running') {
    return 'waiting';
  }
  return 'ready';
}
