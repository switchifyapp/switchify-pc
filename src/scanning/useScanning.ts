import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
export type PointScanConfig = {
  mode: "line" | "grid";
  automatic: boolean;
  speed: number;
  gridSize: number;
  blockIntervalMs: number;
  selectKey: string;
  nextKey: string;
  backKey: string;
  pauseKey: string;
};
export type PointScanState = {
  config: PointScanConfig;
  enabled: boolean;
  phase: "idle" | "row" | "cell" | "x" | "y";
  paused: boolean;
  message: string;
  supported: boolean;
};
export const defaultPointScanConfig: PointScanConfig = {
  mode: "line",
  automatic: true,
  speed: 2,
  gridSize: 4,
  blockIntervalMs: 1000,
  selectKey: "Space",
  nextKey: "Enter",
  backKey: "Backspace",
  pauseKey: "F8",
};

export function validSwitches(config: PointScanConfig) {
  return (
    new Set([
      config.selectKey,
      config.nextKey,
      config.backKey,
      config.pauseKey,
      "Escape",
    ]).size === 5
  );
}
// App owns this hook so changing tabs or views never drops edits or stops a scan.
// Scanning has no toggle: the backend arms it whenever the saved switches allow
// and reports why not through the view's message.
export function useScanning() {
  const [state, setState] = useState<PointScanState | null>(null);
  const [config, setConfig] = useState(defaultPointScanConfig);
  const [pending, setPending] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const model = useRef({
    config: defaultPointScanConfig,
    revision: 0,
    saved: 0,
    pending: 0,
    supported: false,
  });
  const queue = useRef(Promise.resolve());
  const runtimeRevision = useRef(0);
  useEffect(() => {
    let alive = true;
    let stop: (() => void) | undefined;
    const receive = (next: PointScanState) => {
      if (!alive) return;
      model.current.supported = next.supported;
      setState(next);
      if (
        model.current.pending === 0 &&
        model.current.revision === model.current.saved
      ) {
        model.current.config = next.config;
        setConfig(next.config);
      }
    };
    if (!("__TAURI_INTERNALS__" in window)) {
      receive({
        config: defaultPointScanConfig,
        enabled: false,
        phase: "idle",
        paused: false,
        supported: false,
        message: "Open the desktop app to use point scan.",
      });
      return;
    }
    void listen<PointScanState>("point-scan-changed", (event) => {
      runtimeRevision.current++;
      receive(event.payload);
    })
      .then((unlisten) => {
        if (alive) stop = unlisten;
        else unlisten();
        const revision = runtimeRevision.current;
        return invoke<PointScanState>("get_point_scan").then((next) => {
          if (revision === runtimeRevision.current) receive(next);
        });
      })
      .catch((reason) => {
        if (alive) setError(String(reason));
      });
    return () => {
      alive = false;
      stop?.();
    };
  }, []);
  const enqueue = (work: () => Promise<void>) => {
    model.current.pending++;
    setPending(model.current.pending);
    queue.current = queue.current
      .then(work)
      .catch((reason) => setError(String(reason)))
      .finally(() => {
        model.current.pending--;
        setPending(model.current.pending);
      });
  };
  const save = (revision: number, next: PointScanConfig) =>
    enqueue(async () => {
      if (revision !== model.current.revision) return;
      setError(null);
      const runtime = runtimeRevision.current;
      const result = await invoke<PointScanState>("configure_point_scan", {
        config: next,
      });
      model.current.saved = revision;
      if (runtime === runtimeRevision.current) setState(result);
    });
  const update = <K extends keyof PointScanConfig>(
    key: K,
    value: PointScanConfig[K],
  ) => {
    const m = model.current;
    if (!m.supported) return;
    const next = { ...m.config, [key]: value };
    m.config = next;
    m.revision++;
    setConfig(next);
    setError(null);
    if (validSwitches(next)) save(m.revision, next);
  };
  const retry = () => {
    if (validSwitches(model.current.config))
      save(model.current.revision, model.current.config);
  };
  return {
    state,
    config,
    pending,
    error,
    update,
    retry,
    unsaved: model.current.revision !== model.current.saved,
  };
}
export type ScanningController = ReturnType<typeof useScanning>;
