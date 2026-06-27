import type { ReactElement } from 'react';
import type { UpdateState } from '../../shared/update';
import { updateIndicatorState } from '../updates';

type UpdateBannerProps = {
  updateState: UpdateState | null;
  onOpenUpdates: () => Promise<void> | void;
};

export function UpdateBanner({ updateState, onOpenUpdates }: UpdateBannerProps): ReactElement | null {
  const indicator = updateIndicatorState(updateState);
  if (indicator === 'hidden') return null;

  const isReady = indicator === 'downloaded';

  return (
    <section className={`update-banner update-banner-${isReady ? 'ready' : 'available'}`} aria-label="Update status">
      <div>
        <h2 className="update-banner-title">{isReady ? 'Update ready to install' : 'Update available'}</h2>
        <p className="update-banner-body">
          {isReady
            ? 'The update has been downloaded and is ready to install.'
            : 'A new Switchify PC update is ready to download.'}
        </p>
      </div>
      <button type="button" onClick={() => void onOpenUpdates()}>
        Open updates
      </button>
    </section>
  );
}
