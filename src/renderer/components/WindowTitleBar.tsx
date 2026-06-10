import type { ReactElement, ReactNode } from 'react';
import type { DesktopUiState } from '../../shared/desktop-ui-state';

export function WindowChrome({
  title,
  state,
  subtitle,
  className,
  children
}: {
  title: string;
  state?: DesktopUiState;
  subtitle?: string;
  className: string;
  children: ReactNode;
}): ReactElement {
  return (
    <>
      <WindowTitleBar title={title} state={state} subtitle={subtitle} />
      <main className={className}>{children}</main>
    </>
  );
}

export function WindowTitleBar({
  title,
  state,
  subtitle
}: {
  title: string;
  state?: DesktopUiState;
  subtitle?: string;
}): ReactElement {
  return (
    <header className="window-titlebar" aria-label="Window title bar">
      <div className="window-titlebar-main">
        <span className="window-titlebar-app">{title}</span>
        {state ? (
          <span className={`window-titlebar-status window-titlebar-status-${statusTone(state)}`}>
            <span className="window-titlebar-status-dot" />
            {statusLabel(state)}
          </span>
        ) : null}
        {!state && subtitle ? <span className="window-titlebar-status">{subtitle}</span> : null}
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
