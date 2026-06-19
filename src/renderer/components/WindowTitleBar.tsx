import type { ReactElement, ReactNode } from 'react';
import type { DesktopUiState } from '../../shared/desktop-ui-state';
import type { UpdateState } from '../../shared/update';
import { updateIndicatorState } from '../updates';
import { Tooltip } from './Tooltip';

type WindowControlOptions = {
  minimize?: boolean;
};

export function WindowChrome({
  title,
  state,
  subtitle,
  className,
  windowControls,
  updateState,
  onOpenUpdates,
  children
}: {
  title: string;
  state?: DesktopUiState;
  subtitle?: string;
  className: string;
  windowControls?: WindowControlOptions;
  updateState?: UpdateState | null;
  onOpenUpdates?: () => Promise<void> | void;
  children: ReactNode;
}): ReactElement {
  return (
    <>
      <WindowTitleBar
        title={title}
        state={state}
        subtitle={subtitle}
        windowControls={windowControls}
        updateState={updateState}
        onOpenUpdates={onOpenUpdates}
      />
      <main className={className}>{children}</main>
    </>
  );
}

export function WindowTitleBar({
  title,
  state,
  subtitle,
  windowControls,
  updateState,
  onOpenUpdates
}: {
  title: string;
  state?: DesktopUiState;
  subtitle?: string;
  windowControls?: WindowControlOptions;
  updateState?: UpdateState | null;
  onOpenUpdates?: () => Promise<void> | void;
}): ReactElement {
  const controls = {
    minimize: true,
    ...windowControls
  };
  const updateIndicator = updateIndicatorState(updateState ?? null);

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
      <div className="window-titlebar-controls" aria-label="Window controls">
        {updateIndicator !== 'hidden' && onOpenUpdates ? (
          <Tooltip label={updateIndicator === 'downloaded' ? 'Update ready to install' : 'Update available'} placement="bottom">
            <button
              type="button"
              className={`window-titlebar-control window-titlebar-update window-titlebar-update-${updateIndicator}`}
              aria-label="Open updates"
              onClick={() => void onOpenUpdates()}
            >
              <span className="window-control-icon window-control-icon-update" aria-hidden="true" />
            </button>
          </Tooltip>
        ) : null}
        {controls.minimize ? (
          <Tooltip label="Minimize" placement="bottom">
            <button
              type="button"
              className="window-titlebar-control"
              aria-label="Minimize"
              onClick={() => void window.switchifyPc.minimizeWindow()}
            >
              <span className="window-control-icon window-control-icon-minimize" aria-hidden="true" />
            </button>
          </Tooltip>
        ) : null}
        <Tooltip label="Close" placement="bottom">
          <button
            type="button"
            className="window-titlebar-control window-titlebar-control-close"
            aria-label="Close"
            onClick={() => void window.switchifyPc.closeWindow()}
          >
            <span className="window-control-icon window-control-icon-close" aria-hidden="true" />
          </button>
        </Tooltip>
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
