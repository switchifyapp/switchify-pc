import type { ReactElement } from 'react';
import type { UpdateState } from '../../shared/update';
import { canDownloadUpdate, updateCheckMessage, updateDownloadMessage } from '../updates';

type UpdatesPanelProps = {
  state: UpdateState | null;
  isChecking: boolean;
  isDownloading: boolean;
  onCheck: () => Promise<void>;
  onDownload: () => Promise<void>;
  onInstallDownloaded: () => Promise<void>;
};

export function UpdatesPanel({
  state,
  isChecking,
  isDownloading,
  onCheck,
  onDownload,
  onInstallDownloaded
}: UpdatesPanelProps): ReactElement {
  const downloadMessage = updateDownloadMessage(state?.download ?? null);
  const showDownloadButton = canDownloadUpdate(state);
  const showInstallButton = state?.download.status === 'downloaded';

  return (
    <section className="settings-window-section">
      <h2>Updates</h2>
      <div className="technical-list">
        <div className="update-status-row">
          <span>Installed version</span>
          <strong>{state?.info.currentVersion ?? 'Loading...'}</strong>
        </div>
        <div className="update-status-row">
          <span>Status</span>
          <strong>{updateCheckMessage(state?.info ?? null)}</strong>
        </div>
      </div>

      {state?.download.status === 'downloading' ? (
        <div className="update-progress" aria-label={downloadMessage ?? 'Downloading...'}>
          <div style={{ width: `${state.download.percent ?? 100}%` }} />
        </div>
      ) : null}

      {downloadMessage ? <div className={state?.download.status === 'download_failed' ? 'inline-error' : 'empty-state'}>{downloadMessage}</div> : null}

      <div className="technical-list-actions">
        <button type="button" onClick={() => void onCheck()} disabled={isChecking || isDownloading}>
          {isChecking ? 'Checking...' : 'Check for updates'}
        </button>
        {showDownloadButton ? (
          <button type="button" className="primary-button" onClick={() => void onDownload()} disabled={isChecking || isDownloading}>
            {isDownloading ? 'Downloading...' : 'Download update'}
          </button>
        ) : null}
        {showInstallButton ? (
          <button type="button" className="primary-button" onClick={() => void onInstallDownloaded()}>
            Install and restart
          </button>
        ) : null}
      </div>
    </section>
  );
}
