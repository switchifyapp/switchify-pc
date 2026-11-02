import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { CustomPanel } from "../types";

/** The user's panels, kept current while settings sync changes them. A
 *  refresh never lands while an edit is open, which it would discard. */
export function usePanels() {
  const [panels, setPanels] = useState<CustomPanel[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const editing = useRef(false);
  const load = useCallback(() => api.listPanels().then(setPanels).catch((reason) => setError(String(reason))), []);
  useEffect(() => {
    void load();
    const stop = api.onPanelsChanged(() => { if (!editing.current) void load(); });
    return () => { void stop.then((unlisten) => unlisten()); };
  }, [load]);
  const run = async (operation: () => Promise<CustomPanel[]>) => {
    setBusy(true); setError(null);
    try { setPanels(await operation()); }
    catch (reason) { setError(String(reason)); throw reason; }
    finally { setBusy(false); }
  };
  return {
    panels,
    busy,
    error,
    clearError: () => setError(null),
    setEditing: (open: boolean) => { editing.current = open; if (!open) void load(); },
    save: (panel: CustomPanel) => run(() => api.savePanel(panel)),
    remove: (panelId: string) => run(() => api.deletePanel(panelId)),
  };
}
export type PanelsController = ReturnType<typeof usePanels>;
