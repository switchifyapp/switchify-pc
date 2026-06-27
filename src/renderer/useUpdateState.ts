import { useEffect, useState, type Dispatch, type SetStateAction } from 'react';
import type { UpdateState } from '../shared/update';

export function useUpdateState(bridge: Window['switchifyPc']): {
  updateState: UpdateState | null;
  setUpdateState: Dispatch<SetStateAction<UpdateState | null>>;
} {
  const [updateState, setUpdateState] = useState<UpdateState | null>(null);

  useEffect(() => {
    let cancelled = false;
    void bridge.getUpdateState().then((state) => {
      if (!cancelled) {
        setUpdateState(state);
      }
    });

    const unsubscribe = bridge.onUpdateStateChanged((state) => {
      if (!cancelled) {
        setUpdateState(state);
      }
    });

    return () => {
      cancelled = true;
      unsubscribe();
    };
  }, [bridge]);

  return { updateState, setUpdateState };
}
