import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import type { BuiltinPanelView, CustomPanel, PanelCell, PanelExtrasView } from "../types";

/** The user's panels, kept current while settings sync changes them. A
 *  refresh never lands while an edit is open, which it would discard. */
export function usePanels() {
  const [panels, setPanels] = useState<CustomPanel[] | null>(null);
  const [builtins, setBuiltins] = useState<BuiltinPanelView[]>([]);
  const [extras, setExtras] = useState<PanelExtrasView[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const editing = useRef(false);
  // A load that started before a save or a newer load is ignored, so a stale
  // list never overwrites a fresh one.
  const generation = useRef(0);
  const load = useCallback(() => {
    const mine = ++generation.current;
    void api.listBuiltinPanels().then((list) => { if (mine === generation.current) setBuiltins(list); }).catch((reason) => { if (mine === generation.current) setError(String(reason)); });
    void api.listPanelExtras().then((list) => { if (mine === generation.current) setExtras(list); }).catch((reason) => { if (mine === generation.current) setError(String(reason)); });
    return api.listPanels().then((list) => { if (mine === generation.current) setPanels(list); }).catch((reason) => {
      if (mine !== generation.current) return;
      setError(String(reason));
      // A page that never loaded still offers New panel, with the error beside it.
      setPanels((current) => current ?? []);
    });
  }, []);
  useEffect(() => {
    void load();
    const stop = api.onPanelsChanged(() => { if (!editing.current) void load(); });
    return () => { void stop.then((unlisten) => unlisten()); };
  }, [load]);
  // Stable, so the page's unmount cleanup runs once rather than on every render.
  const setEditing = useCallback((open: boolean) => { editing.current = open; if (!open) void load(); }, [load]);
  const run = async <T,>(operation: () => Promise<T>, publish: (result: T) => void) => {
    setBusy(true); setError(null);
    generation.current++;
    try { const result = await operation(); publish(result); return result; }
    catch (reason) { setError(String(reason)); throw reason; }
    finally { setBusy(false); }
  };
  return {
    panels,
    builtins,
    extras,
    busy,
    error,
    clearError: () => setError(null),
    setEditing,
    save: (panel: CustomPanel) => run(() => api.savePanel(panel), setPanels),
    remove: (panelId: string) => run(() => api.deletePanel(panelId), setPanels),
    saveBuiltin: (kind: string, panel: CustomPanel) => run(() => api.saveBuiltinPanel(kind, panel), setBuiltins),
    resetBuiltin: (kind: string) => run(() => api.resetBuiltinPanel(kind), setBuiltins),
    saveExtras: (target: string, rows: PanelCell[][]) => run(() => api.savePanelExtras(target, rows), setExtras),
  };
}
export type PanelsController = ReturnType<typeof usePanels>;
